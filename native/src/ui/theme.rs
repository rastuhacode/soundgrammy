//! Semantic design tokens shared by Kit's theme and custom presentation.
use super::*;
pub(super) const BG: u32 = 0x141414;
pub(super) const CARD: u32 = 0x202020;
pub(super) const SIDEBAR: u32 = 0x191919;
pub(super) const FG: u32 = 0xf2f2f2;
pub(super) const MUTED: u32 = 0xa5a5a5;
pub(super) const PRIMARY: u32 = 0x2aabee;
pub(super) const BORDER: u32 = 0x383838;
pub(super) const SECONDARY: u32 = 0x272727;
pub(super) const PRIMARY_FG: u32 = 0xfafafa;
pub(super) const ON_PRIMARY: u32 = 0x111111;
pub(super) const PRIMARY_HOVER: u32 = 0x2398d5;
pub(super) const ERROR: u32 = 0xff656a;
pub(super) const SELECTED: u32 = 0x203039;
pub(super) const PRIMARY_ACTIVE: u32 = 0x2086bd;

pub(super) fn configure_theme(cx: &mut App) {
    gpui_kit::base::TooltipDefaults::new()
        .with_grace_period(Duration::ZERO)
        .install(cx);
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.colors.primary = rgb(PRIMARY).into();
        theme.colors.primary_foreground = rgb(PRIMARY_FG).into();
        theme.colors.button_primary = rgb(PRIMARY).into();
        theme.colors.button_primary_foreground = rgb(ON_PRIMARY).into();
        theme.colors.primary_hover = rgb(PRIMARY_HOVER).into();
        theme.colors.primary_active = rgb(PRIMARY_ACTIVE).into();
        theme.colors.button_primary_hover = rgb(PRIMARY_HOVER).into();
        theme.colors.button_primary_active = rgb(PRIMARY_ACTIVE).into();
        theme.colors.slider_bar = rgb(PRIMARY).into();
        theme.colors.slider_thumb = rgb(PRIMARY).into();
        theme.radius = px(12.);
        theme.radius_lg = px(16.);
        theme.colors.input = rgb(BORDER).into();
        theme.colors.popover = rgb(CARD).into();
        theme.colors.popover_foreground = rgb(FG).into();
        theme.colors.background = rgb(BG).into();
        theme.colors.foreground = rgb(FG).into();
        theme.colors.secondary = rgb(SECONDARY).into();
        theme.colors.border = rgb(BORDER).into();
        theme.colors.muted_foreground = rgb(MUTED).into();
    });
}

/// Apply alpha to a shared semantic color without duplicating its RGB value.
pub(super) fn tint(color: u32, alpha: u8) -> Rgba {
    rgba((color << 8) | u32::from(alpha))
}
