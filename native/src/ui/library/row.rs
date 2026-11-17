use super::super::menus::MenuGroups;
use super::super::views::*;
use super::super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};

impl PlayerApp {
    pub(in crate::ui) fn track_row(
        &self,
        entry: &Entry,
        index: usize,
        compact: bool,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = entry.track.id;
        let source_index = entry.source_index;
        #[cfg(test)]
        if !active {
            self.inactive_row_renders.update(|count| count + 1);
        }
        let selected = self.library.read(cx).selection.contains(&source_index);
        let label = title(&entry.track);
        let artist_name = artist(&entry.track);
        let view = cx.entity();
        let context_entry = entry.clone();
        let menu_entry = entry.clone();
        let menu_view = view.clone();
        let drag = TrackDrag {
            index: source_index,
            playlist: self.library.read(cx).selected.clone(),
        };
        let label_drag = label.clone();
        let drag_artist = artist_name.clone();
        let drag_duration = entry.track.duration.unwrap_or(0);
        let cover = self.thumbs.get(&id).cloned().flatten();
        let sidebar = self.preferences.sidebar_width;
        let reorderable = self.library.read(cx).reorderable()
            && self.library.read(cx).sort.is_empty()
            && self.library.read(cx).search.is_empty()
            && !self.selection_mode;
        let drag_view = cx.entity();
        let sort_drag = self.sort_drag;
        let playlist = self.library.read(cx).selected.clone();
        let row = gpui_kit::base::TableRow::new(("track-row", source_index), index + 1)
            .group("track-row")
            .aria_selected(selected)
            .aria_label(format!("{} — {}", label, artist_name))
            .w_full()
            .h(px(if compact { 80. } else { 70. }))
            .px(px(if compact { 8. } else { 10. }))
            .py_2()
            .flex()
            .items_center()
            .gap(px(if compact { 8. } else { 12. }))
            .rounded(px(12.))
            .cursor_pointer()
            .focusable()
            .key_context("TrackRow")
            .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    if app.selection_mode {
                        app.library.clone().update(cx, |library, cx| {
                            library.select_entry(source_index, event.keystroke.modifiers.shift);
                            cx.notify();
                        });
                        cx.notify();
                    } else {
                        app.play_entries(index, None, cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .relative()
            .top(px(drag_shift(
                sort_drag,
                SortList::Tracks,
                source_index,
                if compact { 88. } else { 78. },
            )))
            .when(
                sort_drag.is_some_and(|d| d.list == SortList::Tracks && d.from == source_index),
                |el| el.opacity(0.),
            )
            .border_2()
            .border_color(gpui_kit::transparent_black())
            .when(active && !selected, |el| el.bg(rgb(0x1b252b)))
            .when(selected, |el| {
                el.border_color(tint(PRIMARY, 0x80)).bg(tint(PRIMARY, 0x14))
            })
            .hover(|el| el.bg(rgb(CARD)))
            .when(self.selection_mode, |el| {
                el.child(
                    div()
                        .w(px(if compact { 48. } else { 36. }))
                        .flex_shrink_0()
                        .flex()
                        .justify_center()
                        .child(
                            gpui_kit::base::Checkbox::new(("select-track", source_index))
                                .checked(selected)
                                .accessibility_label(format!("Select {}", title(&entry.track)))
                                .child(super::settings::check_mark(
                                    selected,
                                    if compact { 24. } else { 16. },
                                ))
                                .on_change({
                                    let toggle = cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        app.library.clone().update(cx, |library, cx| {
                                            library.select_entry(source_index, false);
                                            cx.notify();
                                        });
                                        cx.stop_propagation();
                                        cx.notify();
                                    });
                                    move |_, event, window, cx| toggle(event, window, cx)
                                }),
                        ),
                )
            })
            .child(
                div()
                    .flex_basis(px(0.))
                    .flex_grow(if compact { 1. } else { 1.4 })
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .size(px(if compact { 56. } else { 48. }))
                            .relative()
                            .flex_shrink_0()
                            .border_1()
                            .border_color(if self.library.read(cx).cached.contains(&id) {
                                rgb(PRIMARY)
                            } else {
                                rgba(0xa3a3a359)
                            })
                            .rounded(px(10.))
                            .bg(rgb(CARD))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(self.thumbs.get(&id).cloned().flatten(), |el, path| {
                                el.child(if active {
                                    if let Some(image) = self.playing_artwork.clone() {
                                        img(image).size_full().rounded(px(10.)).into_any_element()
                                    } else {
                                        img(path)
                                            .image_cache(&self.artwork_cache)
                                            .size_full()
                                            .rounded(px(10.))
                                            .opacity(0.6)
                                            .into_any_element()
                                    }
                                } else {
                                    img(path)
                                        .image_cache(&self.artwork_cache)
                                        .size_full()
                                        .rounded(px(10.))
                                        .into_any_element()
                                })
                            })
                            .when(self.thumbs.get(&id).cloned().flatten().is_none(), |el| {
                                el.child(glyph(IconName::Music, 20.).text_color(rgb(MUTED)))
                            })
                            .when_some(self.download_progress.get(&id).copied(), |el, ratio| {
                                el.child(thumbnail_progress(id, ratio))
                            })
                            .when(true, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(10.))
                                        .bg(rgba(if active { 0x00000000 } else { 0x00000073 }))
                                        .opacity(if active { 1. } else { 0. })
                                        .when(!self.selection_mode, |el| {
                                            el.when(!self.selection_mode, |el| {
                                                el.group_hover("track-row", |el| el.opacity(1.))
                                            })
                                        })
                                        .child(
                                            if active
                                                && self
                                                    .playback
                                                    .read(cx)
                                                    .player
                                                    .as_ref()
                                                    .is_some_and(|p| p.is_playing)
                                            {
                                                self.track_equalizer.clone().into_any_element()
                                            } else {
                                                glyph(IconName::Play, 20.)
                                                    .text_color(rgb(if active {
                                                        PRIMARY
                                                    } else {
                                                        FG
                                                    }))
                                                    .into_any_element()
                                            },
                                        ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .id(("track-title", source_index))
                                    .test_support()
                                    .hover_title(label.clone(), &self.hover_title_epoch)
                                    .truncate()
                                    .when(active, |el| el.text_color(rgb(PRIMARY)))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_sm()
                                    .child(label),
                            )
                            .when(compact, |el| {
                                el.child(
                                    muted(artist_name.clone())
                                        .hover_title(artist_name.clone(), &self.hover_title_epoch)
                                        .text_xs()
                                        .line_height(px(16.))
                                        .truncate(),
                                )
                            }),
                    ),
            )
            .when(!compact, |el| {
                el.child(
                    div()
                        .flex_basis(px(0.))
                        .flex_grow(1.)
                        .min_w_0()
                        .id(("track-artist", source_index))
                        .test_support()
                        .child(
                            muted(artist_name.clone())
                                .truncate()
                                .hover_title(artist_name, &self.hover_title_epoch),
                        ),
                )
            })
            .child(
                muted(
                    entry
                        .track
                        .duration
                        .map(|seconds| format!("{}:{:02}", seconds / 60, seconds % 60))
                        .unwrap_or_else(|| "--:--".into()),
                )
                .w(px(if compact { 48. } else { 72. }))
                .flex_shrink_0()
                .text_right()
                .font_family(mono_font())
                .text_xs(),
            )
            .child(
                div()
                    .id(("track-options", source_index))
                    .w(px(if compact { 48. } else { 36. }))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .when(!compact, |el| el.opacity(0.))
                    .when(!self.selection_mode, |el| {
                        el.group_hover("track-row", |el| el.opacity(1.))
                    })
                    .when(!self.selection_mode, |el| {
                        el.child(
                            icon(("track-menu", source_index), IconName::Ellipsis)
                                .w(px(if compact { 36. } else { 24. }))
                                .dropdown_menu(move |menu, window, cx| {
                                    Self::track_menu(
                                        menu,
                                        &menu_view,
                                        menu_entry.clone(),
                                        window,
                                        cx,
                                    )
                                }),
                        )
                    }),
            )
            .focusable()
            .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if app.selection_mode {
                        app.library.clone().update(cx, |library, cx| {
                            library.select_entry(source_index, event.keystroke.modifiers.shift);
                            cx.notify();
                        });
                    } else {
                        app.play_entries(index, None, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, event: &MouseDownEvent, window, cx| {
                    if !window.last_input_was_touch() {
                        return;
                    }
                    app.touch_generation += 1;
                    let generation = app.touch_generation;
                    app.touch_press = Some((source_index, event.position, generation));
                    cx.spawn(async move |view, cx| {
                        crate::runtime::spawn(async {
                            tokio::time::sleep(Duration::from_millis(500)).await
                        })
                        .await
                        .ok();
                        let _ = view.update(cx, |app, cx| {
                            if app.touch_press.as_ref().is_some_and(|(index, _, token)| {
                                *index == source_index && *token == generation
                            }) {
                                app.touch_press = None;
                                app.suppress_touch_click = Some(source_index);
                                app.selection_mode = true;
                                app.library.clone().update(cx, |library, cx| {
                                    library.select_entry(source_index, false);
                                    cx.notify();
                                });
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }),
            )
            .on_mouse_move(cx.listener(move |app, event: &MouseMoveEvent, _, _| {
                if let Some((index, start, _)) = app.touch_press {
                    if index == source_index
                        && ((f32::from(event.position.x - start.x)).abs() > 8.
                            || (f32::from(event.position.y - start.y)).abs() > 8.)
                    {
                        app.touch_press = None;
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|app, _, _, _| app.touch_press = None),
            )
            .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                if app.suppress_touch_click.take() == Some(source_index) {
                    return;
                }
                if app.selection_mode || event.modifiers().control || event.modifiers().platform {
                    app.selection_mode = true;
                    app.library.clone().update(cx, |library, cx| {
                        library.select_entry(source_index, event.modifiers().shift);
                        cx.notify();
                    });
                    cx.notify();
                } else {
                    app.play_entries(index, None, cx);
                }
            }))
            .when(reorderable, |el| {
                el.on_drag(drag, move |_, _, window, cx| {
                    let artwork_cache = drag_view.update(cx, |app, cx| {
                        app.sort_drag = Some(SortDrag {
                            list: SortList::Tracks,
                            from: source_index,
                            over: source_index,
                        });
                        cx.notify();
                        app.artwork_cache.clone()
                    });
                    let width = f32::from(window.viewport_size().width)
                        - if compact { 16. } else { sidebar + 32. };
                    let start_x = window.mouse_position().x;
                    cx.new(|_| DragPreview {
                        artwork_cache,
                        label: label_drag.clone(),
                        artist: Some(drag_artist.clone()),
                        cover: cover.clone(),
                        width,
                        height: if compact { 80. } else { 70. },
                        start_x,
                        kind: SortList::Tracks,
                        compact,
                        trailing: time(drag_duration as f64),
                    })
                })
            });
        let row = row
            .context_menu(move |menu, window, cx| {
                Self::track_menu(menu, &view, context_entry.clone(), window, cx)
            })
            .into_any_element();
        div()
            .h(px(if compact { 88. } else { 78. }))
            .w_full()
            .when(reorderable, |el| {
                el.on_drag_move(
                    cx.listener(move |app, event: &DragMoveEvent<TrackDrag>, _, cx| {
                        if !event.bounds.contains(&event.event.position) {
                            return;
                        }
                        let drag = event.drag(cx);
                        if drag.playlist == playlist
                            && app.sort_drag.is_none_or(|d| d.over != source_index)
                        {
                            app.sort_drag = Some(SortDrag {
                                list: SortList::Tracks,
                                from: drag.index,
                                over: source_index,
                            });
                            cx.notify();
                        }
                    }),
                )
                .on_drop(cx.listener(move |app, drag: &TrackDrag, _, cx| {
                    app.sort_drag = None;
                    if drag.playlist == app.library.read(cx).selected {
                        app.reorder_tracks(drag.index, source_index, cx);
                    }
                }))
            })
            .child(row)
            .into_any_element()
    }

    pub(in crate::ui) fn track_menu(
        mut menu: PopupMenu,
        view: &Entity<Self>,
        entry: Entry,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let selected = entry.clone();
        let target = view.clone();
        menu = menu
            .min_w(px(208.))
            .max_w(px(280.))
            .group_label("Selection")
            .item(
                PopupMenuItem::new("Select")
                    .icon(menu_icon(IconName::SquareCheck))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |app, cx| {
                            app.selection_mode = true;
                            app.library.clone().update(cx, |library, cx| {
                                let result = { library.selection.insert(selected.source_index) };
                                cx.notify();
                                result
                            });
                            cx.notify();
                        })
                    }),
            )
            .separator()
            .group_label("Queue");
        let cached = view
            .read(cx)
            .library
            .read(cx)
            .cached
            .contains(&entry.track.id);
        let liked = view.read(cx).library.read(cx).liked(entry.track.id);
        let custom = matches!(
            view.read(cx).library.read(cx).selected,
            PlaylistId::Custom(_)
        );
        for (label, action, symbol) in [
            ("Play next", TrackAction::Next, IconName::ListStart),
            ("Add to end", TrackAction::Append, IconName::ListEnd),
            (
                if liked {
                    "Remove from Liked"
                } else {
                    "Add to Liked"
                },
                TrackAction::Like,
                IconName::Heart,
            ),
        ] {
            if matches!(action, TrackAction::Like) {
                menu = menu.separator().group_label("Playlist");
            }
            let target = view.clone();
            let selected = entry.clone();
            menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(symbol)).on_click(
                move |_, _, cx| {
                    target.update(cx, |app, cx| {
                        let entries = if app.selection_mode
                            && app
                                .library
                                .read(cx)
                                .selection
                                .contains(&selected.source_index)
                        {
                            app.library.read(cx).selected_entries()
                        } else {
                            vec![selected.clone()]
                        };
                        app.track_action(action.clone(), entries, cx);
                    })
                },
            ));
        }
        let playlists = view
            .read(cx)
            .library
            .read(cx)
            .playlists
            .as_ref()
            .map(|p| p.custom.clone())
            .unwrap_or_default();
        let target = view.clone();
        let selected = entry.clone();
        menu = menu.submenu_with_icon(
            Some(menu_icon(IconName::ListPlus)),
            "Add to playlist",
            window,
            cx,
            move |mut menu, _, _| {
                let create_target = target.clone();
                let create_entry = selected.clone();
                menu = menu.item(
                    PopupMenuItem::new("Create playlist")
                        .icon(menu_icon(IconName::Plus))
                        .on_click(move |_, window, cx| {
                            create_target.update(cx, |app, cx| {
                                let tracks = if app.selection_mode
                                    && app
                                        .library
                                        .read(cx)
                                        .selection
                                        .contains(&create_entry.source_index)
                                {
                                    app.library
                                        .read(cx)
                                        .selected_entries()
                                        .iter()
                                        .map(|e| e.track.id)
                                        .collect()
                                } else {
                                    vec![create_entry.track.id]
                                };
                                app.open_panel(Panel::NewPlaylist, window, cx);
                                app.new_playlist_tracks = tracks;
                            })
                        }),
                );
                if !playlists.is_empty() {
                    menu = menu.separator();
                }
                for playlist in &playlists {
                    let target = target.clone();
                    let selected = selected.clone();
                    let id = playlist.id;
                    menu = menu.item(PopupMenuItem::new(playlist.name.clone()).on_click(
                        move |_, _, cx| {
                            target.update(cx, |app, cx| {
                                let entries = if app.selection_mode
                                    && app
                                        .library
                                        .read(cx)
                                        .selection
                                        .contains(&selected.source_index)
                                {
                                    app.library.read(cx).selected_entries()
                                } else {
                                    vec![selected.clone()]
                                };
                                app.track_action(TrackAction::AddToPlaylist(id), entries, cx);
                            })
                        },
                    ));
                }
                menu
            },
        );
        if custom {
            let target = view.clone();
            let selected = entry.clone();
            menu = menu.item(
                PopupMenuItem::new("Remove from playlist")
                    .icon(menu_icon(IconName::ListMinus))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |app, cx| {
                            let entries = if app.selection_mode
                                && app
                                    .library
                                    .read(cx)
                                    .selection
                                    .contains(&selected.source_index)
                            {
                                app.library.read(cx).selected_entries()
                            } else {
                                vec![selected.clone()]
                            };
                            app.track_action(TrackAction::Remove, entries, cx);
                        })
                    }),
            );
        }
        menu = menu.separator().group_label("Track");
        for (label, action, symbol) in [
            (
                if cached { "Remove from cache" } else { "Cache" },
                if cached {
                    TrackAction::RemoveCache
                } else {
                    TrackAction::Cache
                },
                if cached {
                    IconName::HardDriveUpload
                } else {
                    IconName::HardDriveDownload
                },
            ),
            ("Download", TrackAction::Export, IconName::Download),
            ("Show info", TrackAction::Info, IconName::Info),
        ] {
            let target = view.clone();
            let selected = entry.clone();
            menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(symbol)).on_click(
                move |_, _, cx| {
                    target.update(cx, |app, cx| {
                        let entries = if app.selection_mode
                            && app
                                .library
                                .read(cx)
                                .selection
                                .contains(&selected.source_index)
                            && !matches!(action, TrackAction::Info)
                        {
                            app.library.read(cx).selected_entries()
                        } else {
                            vec![selected.clone()]
                        };
                        app.track_action(action.clone(), entries, cx);
                    })
                },
            ));
        }
        menu
    }
}
