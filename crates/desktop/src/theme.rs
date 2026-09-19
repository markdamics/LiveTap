//! The "Ginei" design system (Yoru/dark theme, Tsuki accent), imported from
//! `docs/cluade_design.txt` → `Live Catch Webhook Terminal - Desktop.dc.html`.
//! Token values are transcribed 1:1 from that design's
//! `_ds/.../tokens/*.css`; see that file for the source of truth.
//!
//! Kept as a complete token set rather than trimmed to only what the
//! terminal screen currently uses — the workbench screen (Phase 4) and
//! further terminal polish draw from the same palette.
#![allow(dead_code)]

use iced::font::{Family, Weight};
use iced::widget::{container, rule};
use iced::{color, Border, Color, Font, Theme};

// ---- Fonts (bundled JetBrains Mono, OFL-licensed — see assets/fonts/OFL.txt) ----

const FONT_FAMILY: Family = Family::Name("JetBrains Mono");

pub const FONT_REGULAR: Font = Font {
    family: FONT_FAMILY,
    weight: Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const FONT_MEDIUM: Font = Font {
    weight: Weight::Medium,
    ..FONT_REGULAR
};

// ---- Spacing (tokens/spacing.css) ----

pub const SPACE_2: f32 = 4.0;
pub const SPACE_3: f32 = 8.0;
pub const SPACE_4: f32 = 12.0;
pub const SPACE_5: f32 = 16.0;
pub const SPACE_6: f32 = 24.0;
pub const SPACE_7: f32 = 32.0;
pub const CARD_PAD: f32 = 24.0;

// ---- Radius (tokens/radius.css) ----

pub const RADIUS_XS: f32 = 2.0;
pub const RADIUS_SM: f32 = 3.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 10.0;

// ---- Type scale (tokens/typography.css) ----

pub const TEXT_MICRO: f32 = 11.0;
pub const TEXT_BODY_SM: f32 = 13.0;
pub const TEXT_BODY_MD: f32 = 15.0;
pub const TEXT_SUBHEADING: f32 = 18.0;
pub const TRACKING_LABEL: f32 = 0.16;

// ---- Ink / mist / washi / accent ramp (tokens/colors.css, accents.css) ----

fn ink_1000() -> Color {
    color!(0x04060A)
}
fn ink_900() -> Color {
    color!(0x070A0F)
}
fn ink_800() -> Color {
    color!(0x0B1017)
}
fn ink_700() -> Color {
    color!(0x111823)
}
fn ink_400() -> Color {
    color!(0x2E3B4C)
}
fn mist_400() -> Color {
    color!(0x5A6879)
}
fn mist_300() -> Color {
    color!(0x8794A4)
}
fn mist_100() -> Color {
    color!(0xDCE3EB)
}
fn washi_100() -> Color {
    color!(0xF5F7FA)
}
fn a_300() -> Color {
    color!(0xB7D8F0)
}
fn a_400() -> Color {
    color!(0x8CBFE4)
}
fn a_500() -> Color {
    color!(0x5C9BC9)
}
fn shu_400() -> Color {
    color!(0xE4714D)
}
fn matcha_500() -> Color {
    color!(0x6E9E7F)
}
fn kohaku_500() -> Color {
    color!(0xC4963F)
}

fn alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

// ---- Semantic aliases (dark / Yoru + tsuki accent) ----

pub fn bg_canvas() -> Color {
    ink_1000()
}
pub fn bg_base() -> Color {
    ink_900()
}
pub fn surface_card() -> Color {
    ink_800()
}
pub fn surface_raised() -> Color {
    ink_700()
}
pub fn surface_inset() -> Color {
    alpha(ink_800(), 0.5)
}
pub fn border_hairline() -> Color {
    alpha(ink_400(), 0.6)
}
pub fn border_strong() -> Color {
    ink_400()
}
pub fn border_accent() -> Color {
    a_500()
}
pub fn text_strong() -> Color {
    washi_100()
}
pub fn text_body() -> Color {
    mist_100()
}
pub fn text_muted() -> Color {
    mist_300()
}
pub fn text_faint() -> Color {
    mist_400()
}
pub fn text_accent() -> Color {
    a_300()
}
pub fn accent_solid() -> Color {
    a_400()
}
pub fn accent_quiet() -> Color {
    alpha(a_400(), 0.12)
}
pub fn accent_on() -> Color {
    ink_900()
}
pub fn status_success() -> Color {
    matcha_500()
}
pub fn status_warning() -> Color {
    kohaku_500()
}
pub fn status_danger() -> Color {
    shu_400()
}

/// One of the design's five badge/status tones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

impl Tone {
    pub fn color(self) -> Color {
        match self {
            Tone::Neutral => text_muted(),
            Tone::Accent => text_accent(),
            Tone::Success => status_success(),
            Tone::Warning => status_warning(),
            Tone::Danger => status_danger(),
        }
    }

    fn border_color(self) -> Color {
        match self {
            Tone::Neutral => border_hairline(),
            Tone::Accent => alpha(accent_solid(), 0.35),
            Tone::Success => alpha(status_success(), 0.35),
            Tone::Warning => alpha(status_warning(), 0.35),
            Tone::Danger => alpha(status_danger(), 0.35),
        }
    }

    fn background(self) -> Option<Color> {
        match self {
            Tone::Accent => Some(accent_quiet()),
            _ => None,
        }
    }
}

/// `METHOD_TONE` from the design's `.dc.html` script.
pub fn method_tone(method: &str) -> Tone {
    match method.to_ascii_uppercase().as_str() {
        "GET" => Tone::Success,
        "POST" => Tone::Accent,
        "PUT" => Tone::Warning,
        "DELETE" => Tone::Danger,
        _ => Tone::Neutral, // PATCH and anything else
    }
}

/// `METHOD_COLOR` from the design's `.dc.html` script (used for solid method
/// pills in the workbench; the feed/inspector badges use [`method_tone`]).
pub fn method_color(method: &str) -> Color {
    match method.to_ascii_uppercase().as_str() {
        "GET" => status_success(),
        "POST" => accent_solid(),
        "PUT" => status_warning(),
        "DELETE" => status_danger(),
        _ => text_muted(),
    }
}

/// `r.status >= 500 ? 'danger' : r.status >= 400 ? 'warning' : 'success'`.
pub fn status_tone(status: u16) -> Tone {
    if status >= 500 {
        Tone::Danger
    } else if status >= 400 {
        Tone::Warning
    } else {
        Tone::Success
    }
}

pub mod json {
    use super::*;

    pub fn key() -> Color {
        text_accent()
    }
    pub fn string() -> Color {
        status_success()
    }
    pub fn number() -> Color {
        status_warning()
    }
    pub fn boolean() -> Color {
        accent_solid()
    }
    pub fn null() -> Color {
        status_success()
    }
    pub fn bracket() -> Color {
        text_faint()
    }
    pub fn caret() -> Color {
        text_muted()
    }
    pub fn match_background() -> Color {
        accent_quiet()
    }
}

pub fn theme() -> Theme {
    Theme::custom("Ginei".to_string(), palette())
}

fn palette() -> iced::theme::Palette {
    iced::theme::Palette {
        background: bg_canvas(),
        text: text_body(),
        primary: accent_solid(),
        success: status_success(),
        warning: status_warning(),
        danger: status_danger(),
    }
}

/// `.ginei-badge` / `.ginei-badge--{tone}` (components/core/core.css).
pub fn badge_style(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: tone.background().map(Into::into),
        border: Border {
            color: tone.border_color(),
            width: 1.0,
            radius: RADIUS_XS.into(),
        },
        text_color: Some(tone.color()),
        ..container::Style::default()
    }
}

/// Feed request card: `surface-raised`/`border-accent` when selected,
/// `surface-card`/`border-hairline` otherwise, `radius-lg`.
pub fn card_style(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(if selected {
            surface_raised().into()
        } else {
            surface_card().into()
        }),
        border: Border {
            color: if selected {
                border_accent()
            } else {
                border_hairline()
            },
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        ..container::Style::default()
    }
}

/// A bordered, divided row-list box (header rows, diff rows): hairline
/// border, `radius-lg`, clipped.
pub fn bordered_box(_theme: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: border_hairline(),
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        ..container::Style::default()
    }
}

/// The pill in the top bar holding the public hook URL, and search/input
/// chrome: `surface-inset`, hairline border, `radius-sm`.
pub fn inset_field_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(surface_inset().into()),
        border: Border {
            color: border_hairline(),
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..container::Style::default()
    }
}

pub fn sidebar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(bg_canvas().into()),
        ..container::Style::default()
    }
}

pub fn topbar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(bg_canvas().into()),
        ..container::Style::default()
    }
}

/// A single-pixel `border-hairline` divider (sidebar edge, top bar
/// underline, feed/inspector column split): a [`rule`] rather than a
/// `container` border, since container borders can't be single-sided.
pub fn hairline(_theme: &Theme) -> rule::Style {
    rule::Style {
        color: border_hairline(),
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: false,
    }
}
