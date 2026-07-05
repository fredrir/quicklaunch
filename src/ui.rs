//! The spotlight UI: a centered, translucent search pill on a transparent
//! full-screen wlr-layer-shell overlay. Results appear below only once the user
//! types. Colors/geometry come from config + the resolved theme.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use iced::advanced::widget;
use iced::alignment::{Horizontal, Vertical};
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{Column, Space, container, image, mouse_area, row, stack, svg, text, text_input};
use iced::{Color, Element, Event, Length, Padding, Pixels, Task};

use iced_layershell::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::to_layer_message;

use crate::apps::{self, AppEntry, IconResolver};
use crate::config::Config;
use crate::theme::Theme;
use crate::usage::Usage;
use crate::{kde, launch, search, style};

/// Inline magnifier icon (crisp, theme-independent, no external asset).
const SEARCH_ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#9AA0A6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7.5"/><line x1="21" y1="21" x2="16.8" y2="16.8"/></svg>"##;

static INPUT_ID: LazyLock<widget::Id> = LazyLock::new(widget::Id::unique);

/// Launch the layer-shell application. `initial_query` optionally pre-fills the search.
pub fn run(config: Config, initial_query: Option<String>) -> Result<(), iced_layershell::Error> {
    let default_font = resolve_font(&config);
    let text_size = config.font.size();

    application(
        move || Launcher::new(config.clone(), initial_query.clone()),
        namespace,
        Launcher::update,
        Launcher::view,
    )
    .style(Launcher::style)
    .subscription(Launcher::subscription)
    .theme(|_state: &Launcher| iced::Theme::Dark)
    .settings(Settings {
        layer_settings: LayerShellSettings {
            layer: Layer::Overlay,
            anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
            exclusive_zone: 0,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            ..Default::default()
        },
        default_font,
        default_text_size: Pixels(text_size),
        ..Default::default()
    })
    .run()
}

fn namespace() -> String {
    crate::APP_ID.to_string()
}

fn resolve_font(config: &Config) -> iced::Font {
    let family = config.font.family.clone().or_else(kde::font_family);
    match family {
        // Font::with_name needs a 'static name; the process is short-lived, so leak once.
        Some(f) if !f.is_empty() => iced::Font::with_name(Box::leak(f.into_boxed_str())),
        _ => iced::Font::with_name("Noto Sans"),
    }
}

/// One rendered result: an app index plus its (lazily) resolved icon path.
struct ResultRow {
    app: usize,
    icon: Option<PathBuf>,
}

struct Launcher {
    query: String,
    apps: Vec<AppEntry>,
    results: Vec<ResultRow>,
    selected: usize,
    icons: IconResolver,
    usage: Usage,
    config: Config,
    theme: Theme,
    single_click: bool,
    /// Last clicked row, for double-click-to-launch mode.
    last_click: Option<usize>,
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    Changed(String),
    Event(Event),
    /// Click on a specific result (launches, or selects then launches in dbl-click mode).
    Activate(usize),
    /// Dismiss the launcher (Esc / click outside / focus loss).
    Dismiss,
    /// A click landed on the panel — swallow it (so it doesn't dismiss) and keep focus.
    Ignore,
}

impl Launcher {
    fn new(config: Config, initial_query: Option<String>) -> (Self, Task<Message>) {
        let theme = Theme::resolve(&config.theme);
        let single_click = config.behavior.single_click.unwrap_or_else(kde::single_click);
        let icons = IconResolver::new(config.icons.size, config.icons.theme.clone());

        let mut launcher = Self {
            query: String::new(),
            apps: apps::index_apps(),
            results: Vec::new(),
            selected: 0,
            icons,
            usage: Usage::load(),
            config,
            theme,
            single_click,
            last_click: None,
        };
        if let Some(query) = initial_query {
            launcher.set_query(query);
        }
        (launcher, focus_input())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Changed(query) => {
                self.set_query(query);
                Task::none()
            }
            Message::Event(event) => self.handle_event(event),
            Message::Activate(index) => {
                if self.single_click || self.last_click == Some(index) {
                    self.launch_index(index)
                } else {
                    self.selected = index.min(self.results.len().saturating_sub(1));
                    self.last_click = Some(index);
                    Task::none()
                }
            }
            Message::Dismiss => iced_runtime::exit(),
            // Clicking inside the panel keeps the search field focused.
            Message::Ignore => focus_input(),
            // Variants injected by `#[to_layer_message]`; we never emit them.
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        // `listen_with` (not `listen`) so we also receive keys the text field
        // captures — otherwise Escape/Enter never reach us.
        iced::event::listen_with(key_filter)
    }

    /// Transparent surface -> only the centered panel is visible.
    fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: self.theme.text,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let centered = container(mouse_area(self.panel()).on_press(Message::Ignore))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .padding(Padding {
                top: self.config.window.top_offset,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            });

        if self.config.behavior.close_on_click_outside {
            let backdrop = mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::Dismiss);
            stack![backdrop, centered].into()
        } else {
            centered.into()
        }
    }

    /// The centered panel: search pill + (once typed) the results list.
    fn panel(&self) -> Element<'_, Message> {
        let win = &self.config.window;

        let magnifier = svg(svg::Handle::from_memory(SEARCH_ICON))
            .width(Length::Fixed(style::SEARCH_ICON_SIZE))
            .height(Length::Fixed(style::SEARCH_ICON_SIZE));

        let input_style = style::search_input(&self.theme);
        let input = text_input("Search applications…", &self.query)
            .id(INPUT_ID.clone())
            .on_input(Message::Changed)
            .size(self.config.font.size())
            .padding(Padding::ZERO)
            .style(move |_theme, _status| input_style);

        let pill_style = style::panel(&self.theme, win.radius, win.opacity);
        let pill = container(row![magnifier, input].spacing(12).align_y(iced::Center))
            .padding(Padding::from([14.0, 18.0]))
            .width(Length::Fill)
            .style(move |_theme| pill_style);

        let mut root = Column::new()
            .width(Length::Fixed(win.width))
            .spacing(style::GAP)
            .push(pill);

        if !self.query.is_empty() && !self.results.is_empty() {
            let rows: Vec<Element<Message>> = self
                .results
                .iter()
                .enumerate()
                .map(|(i, r)| self.result_row(i, r))
                .collect();

            let list_style = style::panel(&self.theme, win.radius, win.opacity);
            let list = container(Column::with_children(rows).spacing(style::ROW_SPACING))
                .padding(style::PANEL_PADDING)
                .width(Length::Fill)
                .style(move |_theme| list_style);
            root = root.push(list);
        }

        root.into()
    }

    fn result_row(&self, i: usize, r: &ResultRow) -> Element<'_, Message> {
        let app = &self.apps[r.app];
        let size = Length::Fixed(self.config.icons.size as f32);
        let generic = style::generic_icon(&self.theme);

        let icon: Element<Message> = match &r.icon {
            Some(path) if is_svg(path) => svg(svg::Handle::from_path(path))
                .width(size)
                .height(size)
                .into(),
            Some(path) => image(image::Handle::from_path(path))
                .width(size)
                .height(size)
                .into(),
            None => container(Space::new())
                .width(size)
                .height(size)
                .style(move |_theme| generic)
                .into(),
        };

        let mut labels = Column::new()
            .spacing(1)
            .push(text(&app.name).size(style::NAME_FONT_SIZE).color(self.theme.text));
        if let Some(sub) = app.generic_name.as_deref().or(app.comment.as_deref()) {
            labels = labels.push(
                text(sub.to_string())
                    .size(style::MUTED_FONT_SIZE)
                    .color(self.theme.muted),
            );
        }

        let row_style = style::row(&self.theme, i == self.selected);
        let body = container(
            row![icon, labels]
                .spacing(style::ICON_TEXT_SPACING)
                .align_y(iced::Center),
        )
        .width(Length::Fill)
        .height(Length::Fixed(self.config.window.row_height))
        .align_y(Vertical::Center)
        .padding(Padding::from([0.0, 10.0]))
        .style(move |_theme| row_style);

        mouse_area(body).on_press(Message::Activate(i)).into()
    }

    // ---- state transitions -------------------------------------------------

    fn set_query(&mut self, query: String) {
        self.query = query;
        let ranked = search::rank(
            &self.query,
            &self.apps,
            self.config.window.max_results,
            &self.usage,
            self.config.behavior.frequency_ranking,
        );
        let results = ranked
            .into_iter()
            .map(|i| {
                let icon = self.apps[i]
                    .icon
                    .clone()
                    .and_then(|name| self.icons.resolve(&name));
                ResultRow { app: i, icon }
            })
            .collect();
        self.results = results;
        self.selected = 0;
        self.last_click = None;
    }

    fn launch_index(&mut self, index: usize) -> Task<Message> {
        if let Some(r) = self.results.get(index) {
            let app = &self.apps[r.app];
            self.usage.record(&app.desktop_id);
            let _ = launch::launch(app);
        }
        iced_runtime::exit()
    }

    fn move_selection(&mut self, delta: i32) {
        if self.results.is_empty() {
            return;
        }
        let last = self.results.len() as i32 - 1;
        self.selected = (self.selected as i32 + delta).clamp(0, last) as usize;
    }

    fn handle_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                match key {
                    Key::Named(Named::Escape) => return iced_runtime::exit(),
                    Key::Named(Named::Enter) => return self.launch_index(self.selected),
                    Key::Named(Named::ArrowDown) => self.move_selection(1),
                    Key::Named(Named::ArrowUp) => self.move_selection(-1),
                    Key::Named(Named::Tab) => {
                        self.move_selection(if modifiers.shift() { -1 } else { 1 })
                    }
                    Key::Named(Named::PageDown) => self.move_selection(5),
                    Key::Named(Named::PageUp) => self.move_selection(-5),
                    // vim / emacs-style navigation
                    Key::Character(c) if modifiers.control() => match c.as_str() {
                        "n" | "j" => self.move_selection(1),
                        "p" | "k" => self.move_selection(-1),
                        _ => {}
                    },
                    // Everything else (typing, backspace, Ctrl+A/C/V, cursor keys)
                    // is the text field's job — leave it alone.
                    _ => {}
                }
                Task::none()
            }
            Event::Window(iced::window::Event::Unfocused)
                if self.config.behavior.close_on_focus_loss =>
            {
                iced_runtime::exit()
            }
            // Re-assert focus only on (rare) focus-gain — never per keystroke, which
            // used to clear text selections and fight key handling.
            Event::Window(iced::window::Event::Focused | iced::window::Event::Opened { .. }) => {
                focus_input()
            }
            _ => Task::none(),
        }
    }
}

/// Subscription filter: deliver all keyboard events (even ones the text field
/// captures, like Escape/Enter) plus the few window events we act on. Plain
/// `event::listen()` drops captured events and would swallow Escape.
fn key_filter(
    event: Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    match event {
        Event::Keyboard(_)
        | Event::Window(iced::window::Event::Focused)
        | Event::Window(iced::window::Event::Unfocused)
        | Event::Window(iced::window::Event::Opened { .. }) => Some(Message::Event(event)),
        _ => None,
    }
}

/// A `Task` that focuses the search field by id.
fn focus_input() -> Task<Message> {
    iced_runtime::task::widget(widget::operation::focusable::focus(INPUT_ID.clone()))
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
}
