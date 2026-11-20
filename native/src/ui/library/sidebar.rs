use super::super::views::*;
use super::super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};

impl PlayerApp {
    pub(in crate::ui) fn playlist_cover(
        &self,
        id: &PlaylistId,
        compact: bool,
        cx: &App,
    ) -> AnyElement {
        let (from, to, name) = match id {
            PlaylistId::All => (0x0ea5e9, 0x4338ca, IconName::ListMusic),
            PlaylistId::Builtin(id) if id == "liked" => (0xdc00ff, 0x4731d9, IconName::Heart),
            PlaylistId::Builtin(id) if id == "popular" => {
                (0xffad00, 0xe00022, IconName::TrendingUp)
            }
            PlaylistId::Builtin(_) => (0x10b981, 0x0e7490, IconName::Clock),
            _ => (0x64748b, 0x1e293b, IconName::Music),
        };
        let tracks = if let PlaylistId::Custom(id) = id {
            self.library
                .read(cx)
                .playlists
                .as_ref()
                .and_then(|p| p.custom.iter().find(|p| p.id == *id))
                .map(|p| p.track_ids.iter().take(4).copied().collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            vec![]
        };
        let outer = div()
            .size(px(if compact { 48. } else { 36. }))
            .flex_shrink_0()
            .relative()
            .rounded(px(8.))
            .overflow_hidden()
            .bg(linear_gradient(
                135.,
                linear_color_stop(rgb(from), 0.),
                linear_color_stop(rgb(to), 1.),
            ));
        if tracks.is_empty() {
            outer
                .flex()
                .items_center()
                .justify_center()
                .child(if name == IconName::Heart {
                    filled_heart(20.)
                } else {
                    glyph(name, 20.)
                })
                .into_any_element()
        } else {
            outer
                .children(tracks.iter().enumerate().map(|(index, id)| {
                    let count = tracks.len();
                    let left = if count == 3 {
                        if index == 0 {
                            0.
                        } else {
                            0.5
                        }
                    } else if count == 1 {
                        0.
                    } else {
                        (index % 2) as f32 * 0.5
                    };
                    let top = if count == 3 {
                        if index < 2 {
                            0.
                        } else {
                            0.5
                        }
                    } else if count <= 2 {
                        0.
                    } else {
                        (index / 2) as f32 * 0.5
                    };
                    div()
                        .absolute()
                        .left(relative(left))
                        .top(relative(top))
                        .w(relative(if count == 1 { 1. } else { 0.5 }))
                        .h(relative(if count <= 2 || (count == 3 && index == 0) {
                            1.
                        } else {
                            0.5
                        }))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when_some(self.thumbs.get(id).cloned().flatten(), |el, path| {
                            el.child(
                                img(path)
                                    .image_cache(&self.artwork_cache)
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .when(left == 0. && top == 0., |el| el.rounded_tl(px(8.)))
                                    .when(left > 0. && top == 0. || count == 1, |el| {
                                        el.rounded_tr(px(8.))
                                    })
                                    .when(
                                        left == 0.
                                            && (top > 0. || count <= 2 || count == 3 && index == 0),
                                        |el| el.rounded_bl(px(8.)),
                                    )
                                    .when(
                                        left > 0. && (top > 0. || count <= 2) || count == 1,
                                        |el| el.rounded_br(px(8.)),
                                    ),
                            )
                        })
                        .when(self.thumbs.get(id).cloned().flatten().is_none(), |el| {
                            el.child(gpui_kit::component::Icon::new(IconName::Music).size(px(12.)))
                        })
                }))
                .into_any_element()
        }
    }

    pub(in crate::ui) fn sidebar_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compact = window.viewport_size().width < px(768.);
        let control = if compact { 48. } else { 32. };
        let sidebar_width = self.preferences.sidebar_width;
        let items = self
            .library
            .read(cx)
            .items(&self.preferences)
            .into_iter()
            .filter(|p| {
                self.settings.read(cx).statistics_enabled
                    || !matches!(&p.id, PlaylistId::Builtin(id) if id=="popular" || id=="recent")
            })
            .filter(|p| {
                p.name
                    .to_lowercase()
                    .contains(&self.playlist_search.to_lowercase())
            })
            .collect::<Vec<_>>();
        let view = cx.entity();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(SIDEBAR))
            .pt_4()
            .gap_4()
            .child(
                div()
                    .px_4()
                    .flex()
                    .items_center()
                    .h(px(control))
                    .gap(px(if compact { 16. } else { 8. }))
                    .child(
                        icon_size(
                            "navigation",
                            IconName::Menu,
                            if compact { 24. } else { 16. },
                        )
                        .w(px(control))
                        .h(px(control))
                        .mr(px(if compact { 0. } else { 8. }))
                        .accessibility_label("Menu")
                        .hover_title("Menu", &self.hover_title_epoch)
                        .flex_shrink_0()
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.open_panel(Panel::Navigation, window, cx)
                        })),
                    )
                    .child(
                        heading("Library")
                            .test_support()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex_1()
                            .min_w_0()
                            .truncate(),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(if compact { 8. } else { 2. }))
                            .child(self.filters_button(compact, window, cx))
                            .child(
                                icon_size(
                                    "new-playlist",
                                    IconName::Plus,
                                    if compact { 24. } else { 16. },
                                )
                                .w(px(control))
                                .h(px(control))
                                .on_click(cx.listener(
                                    |app, _, window, cx| {
                                        app.open_panel(Panel::NewPlaylist, window, cx)
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .id("playlist-sidebar-scroll")
                    .px_2()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(items.into_iter().enumerate().map(|(index, item)| {
                        let id = item.id.clone();
                        let selected = self.library.read(cx).selected == id;
                        let label = item.name.clone();
                        let preview_count = item.count.to_string();
                        let preview_cover = if let PlaylistId::Custom(id) = item.id {
                            self.library
                                .read(cx)
                                .playlists
                                .as_ref()
                                .and_then(|p| p.custom.iter().find(|p| p.id == id))
                                .and_then(|p| p.track_ids.first())
                                .and_then(|id| self.thumbs.get(id))
                                .cloned()
                                .flatten()
                        } else {
                            None
                        };
                        let context_view = view.clone();
                        let context_id = id.clone();
                        let drag = id.clone();
                        let drop_id = id.clone();
                        let drag_view = view.clone();
                        let sortable = self.preferences.playlist_sort == "custom"
                            && self.playlist_search.is_empty();
                        let row = gpui_kit::base::Button::new(SharedString::from(format!(
                            "playlist-{}",
                            id.key()
                        )))
                        .group("playlist-row")
                        .relative()
                        .top(px(drag_shift(
                            self.sort_drag,
                            SortList::Playlists,
                            index,
                            if compact { 74. } else { 54. },
                        )))
                        .when(
                            self.sort_drag
                                .is_some_and(|d| d.list == SortList::Playlists && d.from == index),
                            |el| el.opacity(0.),
                        )
                        .accessibility_label(format!("Select {} playlist", item.name))
                        .hover_title(item.name.clone(), &self.hover_title_epoch)
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_2()
                        .py_2()
                        .min_h(px(if window.viewport_size().width < px(768.) {
                            72.
                        } else {
                            52.
                        }))
                        .mb(px(2.))
                        .rounded(px(12.))
                        .cursor_pointer()
                        .when(selected, |el| el.bg(rgb(SELECTED)))
                        .hover(|el| el.bg(rgb(CARD)))
                        .child(self.playlist_cover(
                            &item.id,
                            window.viewport_size().width < px(768.),
                            cx,
                        ))
                        .child(
                            div().flex_1().min_w_0().child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(item.name),
                            ),
                        )
                        .child(
                            muted(item.count.to_string())
                                .font_family(mono_font())
                                .text_xs()
                                .font_weight(FontWeight::NORMAL)
                                .flex_shrink_0()
                                .mr(px(-8.)),
                        )
                        .child({
                            let menu_view = view.clone();
                            let menu_id = item.id.clone();
                            div()
                                .id(SharedString::from(format!(
                                    "playlist-option-holder-{}",
                                    menu_id.key()
                                )))
                                .when(window.viewport_size().width >= px(768.), |el| {
                                    el.opacity(0.)
                                })
                                .group_hover("playlist-row", |el| el.opacity(1.))
                                .flex_shrink_0()
                                .w(px(if window.viewport_size().width < px(768.) {
                                    36.
                                } else {
                                    24.
                                }))
                                .child(div().when(item.id != PlaylistId::All, |el| {
                                    el.child(
                                        icon(
                                            SharedString::from(format!(
                                                "playlist-options-{}",
                                                menu_id.key()
                                            )),
                                            IconName::Ellipsis,
                                        )
                                        .w(px(24.))
                                        .h(px(24.))
                                        .dropdown_menu(
                                            move |menu, _, _| {
                                                Self::playlist_menu(
                                                    menu,
                                                    menu_view.clone(),
                                                    menu_id.clone(),
                                                )
                                            },
                                        ),
                                    )
                                }))
                        })
                        .on_click(
                            cx.listener(move |app, _, _, cx| app.select_playlist(id.clone(), cx)),
                        )
                        .when(sortable, |el| {
                            el.on_drag(PlaylistDrag(drag), move |_, _, window, cx| {
                                let artwork_cache = drag_view.update(cx, |app, cx| {
                                    app.sort_drag = Some(SortDrag {
                                        list: SortList::Playlists,
                                        from: index,
                                        over: index,
                                    });
                                    cx.notify();
                                    app.artwork_cache.clone()
                                });
                                let start_x = window.mouse_position().x;
                                cx.new(|_| DragPreview {
                                    artwork_cache,
                                    label: label.clone(),
                                    artist: None,
                                    cover: preview_cover.clone(),
                                    width: if compact {
                                        f32::from(window.viewport_size().width) - 16.
                                    } else {
                                        sidebar_width - 16.
                                    },
                                    height: if compact { 72. } else { 52. },
                                    start_x,
                                    kind: SortList::Playlists,
                                    compact,
                                    trailing: preview_count.clone(),
                                })
                            })
                        })
                        .context_menu(move |menu, _, _| {
                            Self::playlist_menu(menu, context_view.clone(), context_id.clone())
                        })
                        .into_any_element();
                        div()
                            .h(px(if compact { 74. } else { 54. }))
                            .w_full()
                            .when(sortable, |el| {
                                el.on_drag_move(cx.listener(
                                    move |app, event: &DragMoveEvent<PlaylistDrag>, _, cx| {
                                        if !event.bounds.contains(&event.event.position) {
                                            return;
                                        }
                                        let drag = event.drag(cx);
                                        let order = app.library.read(cx).items(&app.preferences);
                                        if let Some(from) =
                                            order.iter().position(|p| p.id == drag.0)
                                        {
                                            if app.sort_drag.is_none_or(|d| d.over != index) {
                                                app.sort_drag = Some(SortDrag {
                                                    list: SortList::Playlists,
                                                    from,
                                                    over: index,
                                                });
                                                cx.notify();
                                            }
                                        }
                                    },
                                ))
                                .on_drop(cx.listener(
                                    move |app, drag: &PlaylistDrag, _, cx| {
                                        app.sort_drag = None;
                                        let mut order = app
                                            .library
                                            .read(cx)
                                            .items(&app.preferences)
                                            .into_iter()
                                            .map(|p| p.id.key())
                                            .collect::<Vec<_>>();
                                        if let (Some(from), Some(to)) = (
                                            order.iter().position(|p| p == &drag.0.key()),
                                            order.iter().position(|p| p == &drop_id.key()),
                                        ) {
                                            let value = order.remove(from);
                                            order.insert(to, value);
                                            app.preferences.custom_order = order;
                                            app.preferences.playlist_sort = "custom".into();
                                            app.save_preferences();
                                            cx.notify();
                                        }
                                    },
                                ))
                            })
                            .child(row)
                    })),
            )
            .into_any_element()
    }

    pub(in crate::ui) fn playlist_menu(
        menu: PopupMenu,
        context_view: Entity<Self>,
        context_id: PlaylistId,
    ) -> PopupMenu {
        let mut menu = menu.min_w(px(176.)).max_w(px(256.));
        if let PlaylistId::Custom(id) = context_id {
            let v = context_view.clone();
            menu = menu.item(
                PopupMenuItem::new("Edit playlist")
                    .icon(menu_icon(IconName::Pencil))
                    .on_click(move |_, window, cx| {
                        v.update(cx, |app, cx| {
                            app.open_panel(Panel::RenamePlaylist(id), window, cx);
                        })
                    }),
            );
            let v = context_view.clone();
            menu = menu.item(
                PopupMenuItem::new("Delete playlist")
                    .icon(menu_icon(IconName::Trash))
                    .on_click(move |_, _, cx| {
                        v.update(cx, |app, cx| {
                            app.show_panel(Panel::Confirm(Confirmation::DeletePlaylist(id)));
                            cx.notify();
                        })
                    }),
            );
        }
        if let PlaylistId::Builtin(ref id) = context_id {
            let id = id.clone();
            let v = context_view.clone();
            menu = menu.item(
                PopupMenuItem::new("Hide playlist")
                    .icon(menu_icon(IconName::EyeOff))
                    .on_click(move |_, _, cx| {
                        v.update(cx, |app, cx| {
                            app.preferences.hidden.insert(id.clone());
                            if app.library.read(cx).selected == PlaylistId::Builtin(id.clone()) {
                                app.library.clone().update(cx, |library, cx| {
                                    library.select(PlaylistId::All);
                                    cx.notify();
                                });
                                app.preferences.selected = "all".into();
                                app.clear_track_search = true;
                            }
                            app.save_preferences();
                            cx.notify();
                        })
                    }),
            );
        }
        if (matches!(context_id, PlaylistId::Custom(_))
            || context_id == PlaylistId::Builtin("liked".into()))
        {
            let v = context_view.clone();
            let id = context_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Export playlist")
                    .icon(menu_icon(IconName::FileDown))
                    .on_click(move |_, _, cx| {
                        v.update(cx, |app, cx| {
                            app.export_recipe(id.clone(), cx);
                        })
                    }),
            );
        }
        menu
    }
}
