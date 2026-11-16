//! The pinned Base Kit has no Menu primitive. Keep the GPUI menu's native
//! keyboard/submenu lifecycle and provide the original Base UI presentation.
use super::*;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
pub(super) trait MenuGroups {
    fn group_label(self, label: &'static str) -> Self;
}
impl MenuGroups for PopupMenu {
    fn group_label(self, label: &'static str) -> Self {
        self.item(
            PopupMenuItem::element(move |_, _| {
                div()
                    .ml(px(-20.))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(super::views::MUTED))
                    .child(label)
            })
            .disabled(true),
        )
    }
}
