//! Design tokens for the "spotlight" aesthetic.
//!
//! All visual constants live here so the look can be tuned in one place. Colors are
//! dark, Breeze-adjacent. Note: wlr-layer-shell surfaces on KWin get per-pixel alpha
//! but *not* blur-behind, so panels are kept fairly opaque for legibility.

use iced::widget::{container, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

// ---- palette ---------------------------------------------------------------

/// Near-opaque panel background (KWin has no blur-behind for layer surfaces).
pub const PANEL_BG: Color = Color::from_rgba(0.070, 0.070, 0.086, 0.94);
/// Primary text.
pub const TEXT: Color = Color::from_rgb(0.925, 0.925, 0.925);
/// Muted secondary text (generic name / comment).
pub const TEXT_MUTED: Color = Color::from_rgb(0.604, 0.627, 0.651);
/// Placeholder text inside the search field.
pub const TEXT_PLACEHOLDER: Color = Color::from_rgb(0.42, 0.44, 0.47);
/// Selected-row background (Breeze accent blue, translucent).
pub const SELECTED_BG: Color = Color::from_rgba(0.239, 0.682, 0.914, 0.20);
/// Faint fill for the generic-icon fallback.
pub const FAINT_FILL: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.06);
/// Hairline border on panels.
pub const HAIRLINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.07);

// ---- geometry --------------------------------------------------------------

pub const PANEL_WIDTH: f32 = 640.0;
pub const PANEL_RADIUS: f32 = 16.0;
pub const ROW_RADIUS: f32 = 10.0;
pub const TOP_OFFSET: f32 = 220.0;

pub const SEARCH_FONT_SIZE: f32 = 20.0;
pub const NAME_FONT_SIZE: f32 = 15.0;
pub const MUTED_FONT_SIZE: f32 = 12.0;
pub const SEARCH_ICON_SIZE: f32 = 22.0;

pub const ICON_SIZE: f32 = 40.0;
pub const ROW_SPACING: f32 = 2.0;

pub const PANEL_PADDING: f32 = 10.0;
pub const GAP: f32 = 12.0;

/// Maximum number of results rendered.
pub const MAX_RESULTS: usize = 8;

// ---- component styles ------------------------------------------------------

const NO_SHADOW: Shadow = Shadow {
    color: Color::TRANSPARENT,
    offset: Vector::new(0.0, 0.0),
    blur_radius: 0.0,
};

/// A floating panel container: rounded, translucent, soft drop shadow.
pub fn panel(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT),
        background: Some(Background::Color(PANEL_BG)),
        border: Border {
            color: HAIRLINE,
            width: 1.0,
            radius: PANEL_RADIUS.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: Vector::new(0.0, 12.0),
            blur_radius: 40.0,
        },
        snap: true,
    }
}

/// A result row's background: accent tint when selected, transparent otherwise.
pub fn row(selected: bool) -> container::Style {
    container::Style {
        text_color: Some(TEXT),
        background: selected.then_some(Background::Color(SELECTED_BG)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: ROW_RADIUS.into(),
        },
        shadow: NO_SHADOW,
        snap: false,
    }
}

/// Fallback tile shown when an app has no resolvable icon.
pub fn generic_icon(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: None,
        background: Some(Background::Color(FAINT_FILL)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: NO_SHADOW,
        snap: false,
    }
}

/// The search field: borderless and transparent so it sits inside the pill.
pub fn search_input(_theme: &Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_PLACEHOLDER,
        value: TEXT,
        selection: SELECTED_BG,
    }
}
