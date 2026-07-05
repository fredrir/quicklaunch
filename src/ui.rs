//! The spotlight UI: a centered, translucent search pill on a transparent
//! full-screen wlr-layer-shell overlay. Results appear below only once the user
//! types.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use iced::advanced::widget;
use iced::alignment::Horizontal;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{Column, Space, container, image, mouse_area, row, svg, text, text_input};
use iced::{Color, Element, Event, Length, Padding, Task, Theme};

use iced_layershell::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::to_layer_message;

use crate::apps::{self, AppEntry, IconResolver};
use crate::{launch, search, style};

/// Inline magnifier icon (crisp, theme-independent, no external asset).
const SEARCH_ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#9AA0A6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7.5"/><line x1="21" y1="21" x2="16.8" y2="16.8"/></svg>"##;

static INPUT_ID: LazyLock<widget::Id> = LazyLock::new(widget::Id::unique);

/// Launch the layer-shell application. Returns when the launcher is dismissed.
/// `initial_query` optionally pre-fills the search field on boot.
pub fn run(initial_query: Option<String>) -> Result<(), iced_layershell::Error> {
    application(
        move || Launcher::new(initial_query.clone()),
        namespace,
        Launcher::update,
        Launcher::view,
    )
        .style(Launcher::style)
        .subscription(Launcher::subscription)
        .theme(|_state: &Launcher| Theme::Dark)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                layer: Layer::Overlay,
                // Full-screen transparent overlay; the panel is centered with layout.
                anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
                exclusive_zone: 0,
                // Grab the keyboard so typing works immediately (like Spotlight).
                keyboard_interactivity: KeyboardInteractivity::Exclusive,
                ..Default::default()
            },
            default_font: iced::Font::with_name("Noto Sans"),
            ..Default::default()
        })
        .run()
}

fn namespace() -> String {
    "kde-app-launcher".to_string()
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
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    Changed(String),
    Event(Event),
    /// Launch the currently-selected result.
    Launch,
    /// Launch a specific result (clicked).
    Activate(usize),
}

impl Launcher {
    fn new(initial_query: Option<String>) -> (Self, Task<Message>) {
        let mut launcher = Self {
            query: String::new(),
            apps: apps::index_apps(),
            results: Vec::new(),
            selected: 0,
            icons: IconResolver::new(),
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
            Message::Launch => self.activate(self.selected),
            Message::Activate(index) => self.activate(index),
            // Variants injected by `#[to_layer_message]`; we never emit them.
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::event::listen().map(Message::Event)
    }

    /// Transparent surface -> only the centered panel is visible.
    fn style(&self, _theme: &Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: style::TEXT,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let magnifier = svg(svg::Handle::from_memory(SEARCH_ICON))
            .width(Length::Fixed(style::SEARCH_ICON_SIZE))
            .height(Length::Fixed(style::SEARCH_ICON_SIZE));

        let input = text_input("Search applications…", &self.query)
            .id(INPUT_ID.clone())
            .on_input(Message::Changed)
            .on_submit(Message::Launch)
            .size(style::SEARCH_FONT_SIZE)
            .padding(Padding::ZERO)
            .style(style::search_input);

        let pill = container(row![magnifier, input].spacing(12).align_y(iced::Center))
            .padding(Padding::from([14.0, 18.0]))
            .width(Length::Fill)
            .style(style::panel);

        let mut root = Column::new()
            .width(Length::Fixed(style::PANEL_WIDTH))
            .spacing(style::GAP)
            .push(pill);

        if !self.query.is_empty() && !self.results.is_empty() {
            let rows: Vec<Element<Message>> = self
                .results
                .iter()
                .enumerate()
                .map(|(i, r)| self.result_row(i, r))
                .collect();

            let list = container(Column::with_children(rows).spacing(style::ROW_SPACING))
                .padding(style::PANEL_PADDING)
                .width(Length::Fill)
                .style(style::panel);
            root = root.push(list);
        }

        container(root)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .padding(Padding::from([style::TOP_OFFSET, 0.0]))
            .into()
    }

    fn result_row(&self, i: usize, r: &ResultRow) -> Element<'_, Message> {
        let app = &self.apps[r.app];
        let size = Length::Fixed(style::ICON_SIZE);

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
                .style(style::generic_icon)
                .into(),
        };

        let mut labels = Column::new()
            .spacing(1)
            .push(text(&app.name).size(style::NAME_FONT_SIZE).color(style::TEXT));
        if let Some(sub) = app.generic_name.as_deref().or(app.comment.as_deref()) {
            labels = labels.push(
                text(sub.to_string())
                    .size(style::MUTED_FONT_SIZE)
                    .color(style::TEXT_MUTED),
            );
        }

        let is_selected = i == self.selected;
        let body = container(row![icon, labels].spacing(14).align_y(iced::Center))
            .width(Length::Fill)
            .padding(Padding::from([6.0, 10.0]))
            .style(move |_theme: &Theme| style::row(is_selected));

        mouse_area(body).on_press(Message::Activate(i)).into()
    }

    // ---- state transitions -------------------------------------------------

    fn set_query(&mut self, query: String) {
        self.query = query;
        let ranked = search::rank(&self.query, &self.apps, style::MAX_RESULTS);
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
    }

    fn activate(&mut self, index: usize) -> Task<Message> {
        if let Some(r) = self.results.get(index) {
            let _ = launch::launch(&self.apps[r.app]);
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
                key: Key::Named(named),
                ..
            }) => {
                match named {
                    Named::Escape => return iced_runtime::exit(),
                    Named::ArrowDown => self.move_selection(1),
                    Named::ArrowUp => self.move_selection(-1),
                    _ => {}
                }
                // Re-assert focus (iced_layershell #367: focus is flaky under
                // layer-shell, so we keep it pinned to the search field).
                focus_input()
            }
            Event::Keyboard(_) | Event::Window(_) => focus_input(),
            _ => Task::none(),
        }
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
