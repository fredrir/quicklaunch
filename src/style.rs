//! Component styles, driven by a resolved [`Theme`]. Geometry that isn't user-facing
//! lives here as constants; user-configurable geometry comes from `config::Window`.

use iced::widget::{container, text_input};
use iced::{Background, Border, Color, Shadow, Vector};

use crate::theme::{Theme, with_alpha};

// Fixed geometry.
pub const NAME_FONT_SIZE: f32 = 15.0;
pub const MUTED_FONT_SIZE: f32 = 12.0;
pub const SEARCH_ICON_SIZE: f32 = 22.0;
pub const ROW_RADIUS: f32 = 10.0;
pub const ROW_SPACING: f32 = 2.0;
pub const PANEL_PADDING: f32 = 10.0;
pub const GAP: f32 = 12.0;
pub const ICON_TEXT_SPACING: f32 = 14.0;
pub const SELECTION_ALPHA: f32 = 0.22;

const NO_SHADOW: Shadow = Shadow {
    color: Color::TRANSPARENT,
    offset: Vector::new(0.0, 0.0),
    blur_radius: 0.0,
};

/// A floating panel: rounded, translucent (per `opacity`), soft drop shadow.
pub fn panel(theme: &Theme, radius: f32, opacity: f32) -> container::Style {
    container::Style {
        text_color: Some(theme.text),
        background: Some(Background::Color(with_alpha(theme.bg, opacity))),
        border: Border {
            color: theme.hairline,
            width: 1.0,
            radius: radius.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: Vector::new(0.0, 12.0),
            blur_radius: 40.0,
        },
        snap: true,
    }
}

/// A result row: accent-tinted when selected, transparent otherwise.
pub fn row(theme: &Theme, selected: bool) -> container::Style {
    container::Style {
        text_color: Some(theme.text),
        background: selected.then_some(Background::Color(with_alpha(theme.accent, SELECTION_ALPHA))),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: ROW_RADIUS.into(),
        },
        shadow: NO_SHADOW,
        snap: false,
    }
}

/// Fallback tile for an app with no resolvable icon.
pub fn generic_icon(theme: &Theme) -> container::Style {
    container::Style {
        text_color: None,
        background: Some(Background::Color(theme.faint)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: NO_SHADOW,
        snap: false,
    }
}

/// The search field: borderless/transparent so it sits inside the pill.
pub fn search_input(theme: &Theme) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        icon: theme.muted,
        placeholder: theme.placeholder,
        value: theme.text,
        selection: with_alpha(theme.selection, SELECTION_ALPHA),
    }
}
