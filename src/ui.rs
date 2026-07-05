use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use iced::advanced::widget;
use iced::alignment::Vertical;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{container, image, mouse_area, pin, responsive, row, stack, svg, text, text_input, Column, Space};
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

const SEARCH_ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#9AA0A6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7.5"/><line x1="21" y1="21" x2="16.8" y2="16.8"/></svg>"##;

static INPUT_ID: LazyLock<widget::Id> = LazyLock::new(widget::Id::unique);

const REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(350);
const REPEAT_INTERVAL: Duration = Duration::from_millis(30);

#[derive(Debug, Clone, PartialEq)]
enum Held {
    Prev,
    Next,
    Backspace,
    Char(String),
}

pub fn run(config: Config, initial_query: Option<String>) -> Result<(), iced_layershell::Error> {
    let default_font = resolve_font(&config);
    let text_size = config.font.size();
    let namespace = if config.behavior.opening_animation {
        crate::APP_ID
    } else {
        // KWin applies its normal-window fade to arbitrary layer-shell namespaces.
        // Its utility classification is intentionally excluded from opening effects.
        "utility"
    };

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

fn resolve_font(config: &Config) -> iced::Font {
    let family = config.font.family.clone().or_else(kde::font_family);
    match family {
        // Font::with_name needs a 'static name; the process is short-lived, so leak once.
        Some(f) if !f.is_empty() => iced::Font::with_name(Box::leak(f.into_boxed_str())),
        _ => iced::Font::with_name("Noto Sans"),
    }
}

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
    last_click: Option<usize>,
    held: Option<(Held, Instant)>,
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    Changed(String),
    Event(Event),
    Activate(usize),
    Dismiss,
    Ignore,
    RepeatTick,
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
            held: None,
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
            Message::Ignore => focus_input(),
            Message::RepeatTick => self.repeat_tick(),
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        let keys = iced::event::listen_with(key_filter);
        if self.held.is_some() {
            iced::Subscription::batch([
                keys,
                iced::time::every(REPEAT_INTERVAL).map(|_| Message::RepeatTick),
            ])
        } else {
            keys
        }
    }

    fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: self.theme.text,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let launcher = responsive(|size| {
            let x = ((size.width - self.config.window.width) / 2.0).max(0.0);
            let y = ((size.height - style::SEARCH_BAR_HEIGHT) / 2.0).max(0.0);

            pin(mouse_area(self.panel()).on_press(Message::Ignore))
                .x(x)
                .y(y)
                .into()
        });

        if self.config.behavior.close_on_click_outside {
            let backdrop = mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
                .on_press(Message::Dismiss);
            stack![backdrop, launcher].into()
        } else {
            launcher.into()
        }
    }

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
            .height(Length::Fixed(style::SEARCH_BAR_HEIGHT))
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
            Event::Keyboard(keyboard::Event::KeyPressed {
                                key, modifiers, text, ..
                            }) => self.on_key_pressed(key, modifiers, text.map(|t| t.to_string())),
            Event::Keyboard(keyboard::Event::KeyReleased { .. }) => {
                self.held = None;
                Task::none()
            }
            Event::Window(iced::window::Event::Unfocused)
            if self.config.behavior.close_on_focus_loss =>
                {
                    iced_runtime::exit()
                }
            Event::Window(iced::window::Event::Focused | iced::window::Event::Opened { .. }) => {
                focus_input()
            }
            _ => Task::none(),
        }
    }

    fn on_key_pressed(
        &mut self,
        key: Key,
        modifiers: iced::keyboard::Modifiers,
        text: Option<String>,
    ) -> Task<Message> {
        let ctrl = modifiers.control();
        let plain = !ctrl && !modifiers.alt() && !modifiers.logo();
        let held: Option<Held> = match &key {
            Key::Named(Named::Escape) => return iced_runtime::exit(),
            Key::Named(Named::Enter) => return self.launch_index(self.selected),
            Key::Named(Named::ArrowDown) => {
                self.move_selection(1);
                Some(Held::Next)
            }
            Key::Named(Named::ArrowUp) => {
                self.move_selection(-1);
                Some(Held::Prev)
            }
            Key::Named(Named::Tab) => {
                let back = modifiers.shift();
                self.move_selection(if back { -1 } else { 1 });
                Some(if back { Held::Prev } else { Held::Next })
            }
            Key::Named(Named::PageDown) => {
                self.move_selection(5);
                None
            }
            Key::Named(Named::PageUp) => {
                self.move_selection(-5);
                None
            }
            Key::Named(Named::Backspace) => Some(Held::Backspace),
            Key::Character(c) if ctrl => match c.as_str() {
                "n" | "j" => {
                    self.move_selection(1);
                    Some(Held::Next)
                }
                "p" | "k" => {
                    self.move_selection(-1);
                    Some(Held::Prev)
                }
                _ => None,
            },
            _ if plain => text
                .filter(|t| !t.is_empty() && !t.chars().all(char::is_control))
                .map(Held::Char),
            _ => None,
        };
        self.held = held.map(|h| (h, Instant::now()));
        Task::none()
    }

    fn repeat_tick(&mut self) -> Task<Message> {
        let Some((held, since)) = self.held.clone() else {
            return Task::none();
        };
        if since.elapsed() < REPEAT_INITIAL_DELAY {
            return Task::none();
        }
        match held {
            Held::Next => self.move_selection(1),
            Held::Prev => self.move_selection(-1),
            Held::Backspace => {
                if self.query.is_empty() {
                    return Task::none();
                }
                let mut query = self.query.clone();
                query.pop();
                self.set_query(query);
                return cursor_to_end();
            }
            Held::Char(text) => {
                self.set_query(format!("{}{}", self.query, text));
                return cursor_to_end();
            }
        }
        Task::none()
    }
}

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

fn focus_input() -> Task<Message> {
    iced_runtime::task::widget(widget::operation::focusable::focus(INPUT_ID.clone()))
}

fn cursor_to_end() -> Task<Message> {
    iced_runtime::task::widget(widget::operation::text_input::move_cursor_to_end(INPUT_ID.clone()))
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
}
