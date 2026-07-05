use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use iced::advanced::widget;
use iced::alignment::Vertical;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::mouse;
use iced::widget::{
    Column, Row, Space, container, image, mouse_area, pin, responsive, stack, svg, text, text_input,
};
use iced::{Color, Element, Event, Length, Padding, Pixels, Task};

use iced_layershell::actions::ActionCallback;
use iced_layershell::application;
use iced_layershell::build_pattern::daemon;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::to_layer_message;

use crate::config::{Config, HorizontalPlacement, ResultsPlacement, VerticalPlacement};
use crate::entry::Entry;
use crate::theme::Theme;
use crate::usage::Usage;
use crate::{executor, kde, launch, providers, resident, search, style};

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
    let namespace = namespace(&config);

    application(
        move || Launcher::new(config.clone(), initial_query.clone(), false),
        namespace,
        Launcher::update,
        Launcher::view,
    )
    .style(Launcher::style)
    .subscription(Launcher::subscription)
    .theme(|_state: &Launcher| iced::Theme::Dark)
    .settings(settings(default_font, text_size))
    .executor::<executor::Executor>()
    .run()
}

pub fn run_resident(
    config: Config,
    initial_query: Option<String>,
) -> Result<(), iced_layershell::Error> {
    let default_font = resolve_font(&config);
    let text_size = config.font.size();
    let namespace = namespace(&config).to_string();
    let mut daemon_settings = settings(default_font, text_size);
    daemon_settings.layer_settings.size = Some((0, 0));

    daemon(
        move || Launcher::new(config.clone(), initial_query.clone(), true),
        move || namespace.clone(),
        Launcher::update,
        daemon_view,
    )
    .style(|state, theme| state.style(theme))
    .subscription(Launcher::subscription)
    .theme(|_state: &Launcher, _window| iced::Theme::Dark)
    .settings(daemon_settings)
    .executor::<executor::Executor>()
    .run()
}

fn daemon_view(state: &Launcher, _window: iced::window::Id) -> Element<'_, Message> {
    state.view()
}

fn namespace(config: &Config) -> &'static str {
    if config.behavior.opening_animation {
        crate::APP_ID
    } else {
        "utility"
    }
}

fn settings(default_font: iced::Font, text_size: f32) -> Settings {
    Settings {
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
    }
}

fn new_layer_settings(namespace: String) -> NewLayerShellSettings {
    NewLayerShellSettings {
        layer: Layer::Overlay,
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        exclusive_zone: Some(0),
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        output_option: OutputOption::None,
        namespace: Some(namespace),
        ..Default::default()
    }
}

fn resolve_font(config: &Config) -> iced::Font {
    let family = config.font.family.clone().or_else(kde::font_family);
    match family {
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
    apps: Vec<Entry>,
    results: Vec<ResultRow>,
    selected: usize,
    usage: Usage,
    config: Config,
    theme: Theme,
    single_click: bool,
    last_click: Option<usize>,
    held: Option<(Held, Instant)>,
    resident: bool,
    visible: bool,
    active_window: Option<iced::window::Id>,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
enum Message {
    Changed(String),
    EntriesLoaded(Vec<Entry>),
    Event(iced::window::Id, Event),
    Activate(usize),
    Dismiss,
    Ignore,
    RepeatTick,
    Persisted,
    ResidentToggle(Option<String>),
    WindowOpened(iced::window::Id),
    WindowClosed(iced::window::Id),
}

impl Launcher {
    fn new(config: Config, initial_query: Option<String>, resident: bool) -> (Self, Task<Message>) {
        let theme = Theme::resolve(&config.theme);
        let single_click = config
            .behavior
            .single_click
            .unwrap_or_else(kde::single_click);
        let plugin_config = config.plugins.clone();
        let icon_size = if config.icons.show {
            config.icons.size
        } else {
            0
        };
        let icon_theme = config.icons.theme.clone();
        let plugin_icon_theme = config.icons.theme.clone();

        let launcher = Self {
            query: initial_query.unwrap_or_default(),
            apps: Vec::new(),
            results: Vec::new(),
            selected: 0,
            usage: Usage::load(),
            config,
            theme,
            single_click,
            last_click: None,
            held: None,
            resident,
            visible: true,
            active_window: None,
        };
        let mut startup = vec![
            focus_input(),
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        providers::load_applications(icon_size, icon_theme)
                    })
                    .await
                    .unwrap_or_default()
                },
                Message::EntriesLoaded,
            ),
        ];
        startup.extend(
            plugin_config
                .into_iter()
                .filter(|plugin| plugin.enabled)
                .map(|plugin| {
                    let icon_theme = plugin_icon_theme.clone();
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                providers::load_plugin(plugin, icon_size, icon_theme)
                            })
                            .await
                            .unwrap_or_default()
                        },
                        Message::EntriesLoaded,
                    )
                }),
        );
        (launcher, Task::batch(startup))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Changed(query) => {
                self.set_query(query);
                Task::none()
            }
            Message::EntriesLoaded(mut apps) => {
                self.apps.append(&mut apps);
                self.set_query(self.query.clone());
                Task::none()
            }
            Message::Event(window, event) => self.handle_event(window, event),
            Message::Activate(index) => {
                if self.single_click || self.last_click == Some(index) {
                    self.launch_index(index)
                } else {
                    self.selected = index.min(self.results.len().saturating_sub(1));
                    self.last_click = Some(index);
                    Task::none()
                }
            }
            Message::Dismiss => self.dismiss(),
            Message::Ignore => focus_input(),
            Message::RepeatTick => self.repeat_tick(),
            Message::Persisted => {
                if self.resident {
                    Task::none()
                } else {
                    iced_runtime::exit()
                }
            }
            Message::ResidentToggle(query) => self.toggle(query),
            Message::WindowOpened(id) => {
                self.active_window = Some(id);
                self.visible = true;
                focus_input()
            }
            Message::WindowClosed(id) => {
                if self.active_window == Some(id) {
                    self.active_window = None;
                    self.visible = false;
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        let keys = iced::event::listen_with(key_filter);
        let mut subscriptions = vec![
            keys,
            iced::window::open_events().map(Message::WindowOpened),
            iced::window::close_events().map(Message::WindowClosed),
        ];
        if self.resident {
            subscriptions.push(resident::subscription().map(Message::ResidentToggle));
        }
        if self.held.is_some() {
            subscriptions.push(iced::time::every(REPEAT_INTERVAL).map(|_| Message::RepeatTick));
        }
        iced::Subscription::batch(subscriptions)
    }

    fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: self.theme.text,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        if self.resident && !self.visible {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        }
        let launcher = responsive(|size| {
            let placement = &self.config.placement;
            let width = self.config.window.width.min(size.width).max(1.0);
            let height = self.config.input.height.min(size.height).max(1.0);
            let margin = placement.margin.max(0.0);
            let base_x = match placement.horizontal {
                HorizontalPlacement::Left => margin,
                HorizontalPlacement::Center => (size.width - width) / 2.0,
                HorizontalPlacement::Right => size.width - width - margin,
            };
            let base_y = match placement.vertical {
                VerticalPlacement::Top => margin,
                VerticalPlacement::Center => (size.height - height) / 2.0,
                VerticalPlacement::Bottom => size.height - height - margin,
            };
            let x = (base_x + placement.x_offset).clamp(0.0, (size.width - width).max(0.0));
            let search_y =
                (base_y + placement.y_offset).clamp(0.0, (size.height - height).max(0.0));
            let y = match placement.results {
                ResultsPlacement::Below => search_y,
                ResultsPlacement::Above => search_y - self.results_stack_height(),
            };

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

        let input_style = style::search_input(&self.theme);
        let input = text_input(&self.config.input.placeholder, &self.query)
            .id(INPUT_ID.clone())
            .on_input(Message::Changed)
            .size(
                self.config
                    .input
                    .font_size
                    .unwrap_or_else(|| self.config.font.size()),
            )
            .padding(Padding::ZERO)
            .style(move |_theme, _status| input_style);

        let mut input_row = Row::new().align_y(iced::Center);
        if self.config.input.show_search_icon {
            let icon_size = Length::Fixed(self.config.input.search_icon_size.max(1.0));
            input_row = input_row
                .push(
                    svg(svg::Handle::from_memory(SEARCH_ICON))
                        .width(icon_size)
                        .height(icon_size),
                )
                .spacing(self.config.input.icon_spacing.max(0.0));
        }
        input_row = input_row.push(input);

        let pill_style = style::panel(&self.theme, win.radius, win.opacity);
        let pill = container(input_row)
            .padding(Padding::from([
                self.config.input.padding_vertical.max(0.0),
                self.config.input.padding_horizontal.max(0.0),
            ]))
            .width(Length::Fill)
            .height(Length::Fixed(self.config.input.height.max(1.0)))
            .style(move |_theme| pill_style);

        let mut result_list = None;

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
            result_list = Some(list);
        }

        let mut root = Column::new()
            .width(Length::Fixed(win.width))
            .spacing(style::GAP);
        match (self.config.placement.results, result_list) {
            (ResultsPlacement::Above, Some(list)) => {
                root = root.push(list).push(pill);
            }
            (_, Some(list)) => {
                root = root.push(pill).push(list);
            }
            (_, None) => {
                root = root.push(pill);
            }
        }

        root.into()
    }

    fn results_stack_height(&self) -> f32 {
        if self.query.is_empty() || self.results.is_empty() {
            return 0.0;
        }
        let rows = self.results.len() as f32;
        let spacing = self.results.len().saturating_sub(1) as f32 * style::ROW_SPACING;
        rows * self.config.window.row_height + spacing + style::PANEL_PADDING * 2.0 + style::GAP
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
            None if self.config.icons.show_fallback => container(Space::new())
                .width(size)
                .height(size)
                .style(move |_theme| generic)
                .into(),
            None => Space::new().width(size).height(size).into(),
        };

        let mut labels = Column::new().spacing(1).push(
            text(&app.name)
                .size(style::NAME_FONT_SIZE)
                .color(self.theme.text),
        );
        if let Some(sub) = app.generic_name.as_deref().or(app.comment.as_deref()) {
            labels = labels.push(
                text(sub.to_string())
                    .size(style::MUTED_FONT_SIZE)
                    .color(self.theme.muted),
            );
        }

        let row_style = style::row(&self.theme, i == self.selected);
        let mut contents = Row::new().align_y(iced::Center);
        if self.config.icons.show {
            contents = contents
                .push(icon)
                .spacing(self.config.icons.spacing.max(0.0));
        }
        contents = contents.push(labels);

        let body = container(contents)
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
                let icon = self
                    .config
                    .icons
                    .show
                    .then(|| self.apps[i].icon_path.clone())
                    .flatten();
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
            if launch::launch(app).is_ok() {
                self.usage.record(&app.id);
                let usage = self.usage.clone();
                let persist = Task::perform(
                    async move {
                        let _ = tokio::task::spawn_blocking(move || usage.save()).await;
                    },
                    |_| Message::Persisted,
                );
                if self.resident {
                    return Task::batch([self.dismiss(), persist]);
                }
                return persist;
            }
        }
        self.dismiss()
    }

    fn dismiss(&mut self) -> Task<Message> {
        if !self.resident {
            return iced_runtime::exit();
        }
        self.visible = false;
        self.held = None;
        if let Some(id) = self.active_window {
            Task::batch([
                Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::None,
                }),
                Task::done(Message::SetInputRegion {
                    id,
                    callback: ActionCallback::new(|_| {}),
                }),
            ])
        } else {
            Task::none()
        }
    }

    fn toggle(&mut self, query: Option<String>) -> Task<Message> {
        if self.visible {
            return self.dismiss();
        }
        self.query = query.unwrap_or_default();
        self.set_query(self.query.clone());
        self.visible = true;
        if let Some(id) = self.active_window {
            return Task::batch([
                Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                }),
                Task::done(Message::SetInputRegion {
                    id,
                    callback: ActionCallback::new(|region| {
                        region.add(0, 0, i32::MAX, i32::MAX);
                    }),
                }),
                focus_input(),
            ]);
        }
        let id = iced::window::Id::unique();
        self.active_window = Some(id);
        Task::done(Message::NewLayerShell {
            settings: new_layer_settings(namespace(&self.config).to_string()),
            id,
        })
    }

    fn move_selection(&mut self, delta: i32) {
        if self.results.is_empty() {
            return;
        }
        self.selected = wrapped_index(self.selected, delta, self.results.len());
    }

    fn handle_event(&mut self, window: iced::window::Id, event: Event) -> Task<Message> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => self.on_key_pressed(key, modifiers, text.map(|t| t.to_string())),
            Event::Keyboard(keyboard::Event::KeyReleased { .. }) => {
                self.held = None;
                Task::none()
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
                };
                if y < 0.0 {
                    self.move_selection(1);
                } else if y > 0.0 {
                    self.move_selection(-1);
                }
                Task::none()
            }
            Event::Window(iced::window::Event::Unfocused)
                if self.config.behavior.close_on_focus_loss =>
            {
                self.dismiss()
            }
            Event::Window(iced::window::Event::Focused | iced::window::Event::Opened { .. }) => {
                self.active_window = Some(window);
                self.visible = true;
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
            Key::Named(Named::Escape) => return self.dismiss(),
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
    window: iced::window::Id,
) -> Option<Message> {
    match event {
        Event::Keyboard(_)
        | Event::Mouse(mouse::Event::WheelScrolled { .. })
        | Event::Window(iced::window::Event::Focused)
        | Event::Window(iced::window::Event::Unfocused)
        | Event::Window(iced::window::Event::Opened { .. }) => Some(Message::Event(window, event)),
        _ => None,
    }
}

fn focus_input() -> Task<Message> {
    iced_runtime::task::widget(widget::operation::focusable::focus(INPUT_ID.clone()))
}

fn cursor_to_end() -> Task<Message> {
    iced_runtime::task::widget(widget::operation::text_input::move_cursor_to_end(
        INPUT_ID.clone(),
    ))
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
}

fn wrapped_index(current: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as i64 + delta as i64).rem_euclid(len as i64) as usize
}

#[cfg(test)]
mod tests {
    use super::wrapped_index;

    #[test]
    fn selection_wraps_in_both_directions() {
        assert_eq!(wrapped_index(2, 1, 3), 0);
        assert_eq!(wrapped_index(0, -1, 3), 2);
        assert_eq!(wrapped_index(1, 5, 3), 0);
        assert_eq!(wrapped_index(0, -5, 3), 1);
        assert_eq!(wrapped_index(0, 1, 0), 0);
    }
}
