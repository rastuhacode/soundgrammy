use super::super::menus::MenuGroups;
use super::super::views::*;
use super::super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    Disableable, Sizable,
};

impl PlayerApp {
    pub(in crate::ui) fn indexing_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (done, total) = self.sync_progress.unwrap_or_else(|| {
            self.library
                .read(cx)
                .status
                .as_ref()
                .map(|s| (s.processed, s.total))
                .unwrap_or_default()
        });
        let fraction = if total > 0 {
            (done as f32 / total as f32).clamp(0., 1.)
        } else {
            0.3
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div().p_4().child(
                    icon("navigation", IconName::Menu)
                        .accessibility_label("Menu")
                        .hover_title("Menu", &self.hover_title_epoch)
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.open_panel(Panel::Navigation, window, cx)
                        })),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .child(
                        div()
                            .w(px(384.))
                            .max_w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_4()
                            .text_center()
                            .child(motion::Layer::bind(
                                "indexing-spinner-layer",
                                window,
                                cx,
                                |_, _, _| {
                                    gpui_kit::component::spinner::Spinner::new()
                                        .icon(IconName::LoaderCircle)
                                        .color(rgb(PRIMARY).into())
                                        .with_size(px(32.))
                                        .into_any_element()
                                },
                            ))
                            .child(heading("Indexing your library").text_lg())
                            .child(muted(if total > 0 {
                                format!("{done} of {total} tracks indexed")
                            } else {
                                "Preparing your library…".into()
                            }))
                            .child(
                                div()
                                    .id("indexing-progress")
                                    .test_support()
                                    .role(Role::ProgressIndicator)
                                    .aria_label("Library indexing progress")
                                    .h(px(8.))
                                    .w_full()
                                    .rounded_full()
                                    .bg(rgb(SECONDARY))
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(fraction))
                                            .rounded_full()
                                            .bg(rgb(PRIMARY)),
                                    ),
                            )
                            .child(muted("Your tracks will appear when indexing finishes."))
                            .when(self.error.is_some(), |el| {
                                el.child(
                                    button("index-retry", "Retry indexing")
                                        .disabled(self.syncing)
                                        .on_click(cx.listener(|app, _, _, cx| app.sync(false, cx))),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(in crate::ui) fn empty_playlist_view(&self, cx: &App) -> AnyElement {
        let (title, description) = if self.library.read(cx).tracks.is_empty() {
            (
                "No tracks yet",
                "Pin music to your Telegram profile and it will tune in here automatically.",
            )
        } else {
            match &self.library.read(cx).selected {
                PlaylistId::Builtin(id) if id == "liked" => (
                    "No liked tracks yet",
                    "Tap the heart on any track to save it here.",
                ),
                PlaylistId::Builtin(id) if id == "popular" || id == "recent" => (
                    "No listening history yet",
                    "Play some tracks and they will show up here.",
                ),
                PlaylistId::Custom(_) => (
                    "This playlist is empty",
                    "Add tracks from your library using the list button.",
                ),
                _ => (
                    "No tracks yet",
                    "Pin music to your Telegram profile and it will tune in here automatically.",
                ),
            }
        };
        div()
            .id("playlist-empty-state")
            .test_support()
            .w_full()
            .px(px(24.))
            .py(px(80.))
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .text_center()
            .child(
                div()
                    .size(px(56.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CARD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph(IconName::Music, 24.).text_color(rgb(MUTED))),
            )
            .child(
                heading(title)
                    .id("playlist-empty-title")
                    .test_support()
                    .font_weight(FontWeight::MEDIUM),
            )
            .child(muted(description).max_w(px(320.)))
            .into_any_element()
    }

    pub(in crate::ui) fn library_view(
        &mut self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if matches!(self.panel, Some(Panel::Search)) {
            return div().size_full().into_any_element();
        }
        let list = self.library.read(cx).track_list();
        let entries = list.entries;
        let count = entries.len();
        let total = list.membership.len();
        let visible = entries.clone();
        let active_row = self
            .library
            .read(cx)
            .playing_source_index(self.playback.read(cx));
        let has_selection = self.selection_mode
            && !self.library.read(cx).selection.is_empty()
            && entries.iter().any(|entry| {
                self.library
                    .read(cx)
                    .selection
                    .contains(&entry.source_index)
            });
        // Match develop's compact toolbar: shuffle appears at 480px,
        // cache/download at 640px; narrower windows keep them in the menu.
        let width = window.viewport_size().width;
        let show_shuffle = !compact || width >= px(480.);
        let show_storage_actions = !compact || width >= px(640.);
        let can_download = !matches!(&self.library.read(cx).selected, PlaylistId::Builtin(id) if id == "popular" || id == "recent");
        let cache_disabled = count == 0
            || entries
                .iter()
                .all(|entry| self.library.read(cx).cached.contains(&entry.track.id));
        let button_size = if compact { 48. } else { 32. };
        let symbol_size = if compact { 24. } else { 16. };
        let actions = div()
            .flex()
            .items_center()
            .gap(px(if compact { 4. } else { 10. }))
            .when(!compact, |el| el.w_full())
            .when(compact, |el| el.flex_shrink_0())
            .when(!self.selection_mode && total > 0, |el| {
                el.child(
                    Button::new("play-playlist")
                        .primary()
                        .child(glyph(IconName::Play, symbol_size))
                        .size(px(button_size))
                        .p_0()
                        .accessibility_label("Play playlist")
                        .hover_title("Play", &self.hover_title_epoch)
                        .disabled(count == 0)
                        .on_click(cx.listener(|app, _, _, cx| app.play_entries(0, None, cx))),
                )
                .when(show_shuffle, |el| {
                    el.child(
                        icon_size("shuffle-playlist", IconName::Shuffle, symbol_size)
                            .size(px(button_size))
                            .hover_title("Shuffle", &self.hover_title_epoch)
                            .bg(rgb(CARD))
                            .rounded(px(12.))
                            .disabled(count == 0)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.play_entries(0, Some(audio::session::Shuffle::On), cx)
                            })),
                    )
                })
                .when(show_storage_actions, |el| {
                    el.child(
                        icon_size("cache-playlist", IconName::HardDriveDownload, symbol_size)
                            .size(px(button_size))
                            .hover_title("Cache playlist", &self.hover_title_epoch)
                            .bg(rgb(CARD))
                            .rounded(px(12.))
                            .disabled(cache_disabled)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.track_action(
                                    TrackAction::Cache,
                                    app.library.read(cx).entries(),
                                    cx,
                                )
                            })),
                    )
                    .when(can_download, |el| {
                        el.child(
                            icon_size("download-playlist", IconName::Download, symbol_size)
                                .size(px(button_size))
                                .hover_title("Download playlist", &self.hover_title_epoch)
                                .bg(rgb(CARD))
                                .rounded(px(12.))
                                .disabled(count == 0)
                                .on_click(cx.listener(|app, _, _, cx| app.download_playlist(cx))),
                        )
                    })
                })
                .when(!show_storage_actions, |el| {
                    let view = cx.entity();
                    el.child(
                        icon_size("more-playlist-actions", IconName::Ellipsis, 20.)
                            .size(px(button_size))
                            .accessibility_label("More playlist actions")
                            .hover_title("More playlist actions", &self.hover_title_epoch)
                            .dropdown_menu(move |mut menu, _, _| {
                                menu = menu.min_w(px(192.));
                                if !show_shuffle {
                                    let view = view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new("Shuffle")
                                            .icon(menu_icon(IconName::Shuffle))
                                            .disabled(count == 0)
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |app, cx| {
                                                    app.play_entries(
                                                        0,
                                                        Some(audio::session::Shuffle::On),
                                                        cx,
                                                    )
                                                });
                                            }),
                                    );
                                }
                                let cache_view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new("Cache playlist")
                                        .icon(menu_icon(IconName::HardDriveDownload))
                                        .disabled(cache_disabled)
                                        .on_click(move |_, _, cx| {
                                            cache_view.update(cx, |app, cx| {
                                                app.track_action(
                                                    TrackAction::Cache,
                                                    app.library.read(cx).entries(),
                                                    cx,
                                                )
                                            });
                                        }),
                                );
                                if can_download {
                                    let view = view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new("Download playlist")
                                            .icon(menu_icon(IconName::Download))
                                            .disabled(count == 0)
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |app, cx| {
                                                    app.download_playlist(cx)
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                })
            })
            .when(self.selection_mode && total > 0, |el| {
                el.when(has_selection, |el| {
                    el.child(self.bulk_actions_button(compact, cx))
                })
                .child(
                    icon_size("exit-selection", IconName::Undo2, symbol_size)
                        .size(px(button_size))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .rounded(px(12.))
                        .accessibility_label("Exit selection")
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.selection_mode = false;
                            app.library.clone().update(cx, |library, cx| {
                                library.clear_selection();
                                cx.notify();
                            });
                            cx.notify();
                        })),
                )
            })
            .when(!compact, |el| {
                el.child(
                    div().flex_1().min_w_0().child(
                        Input::new(&self.inputs.search)
                            .cleanable(true)
                            .prefix(gpui_kit::component::Icon::new(IconName::Search))
                            .id("track-search")
                            .aria_label("Search tracks"),
                    ),
                )
            })
            .when(compact, |el| {
                el.child(
                    icon_size("open-track-search", IconName::Search, 24.)
                        .size(px(48.))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .rounded(px(16.))
                        .accessibility_label("Search tracks")
                        .hover_title(
                            if self.library.read(cx).search.is_empty() {
                                "Search tracks".into()
                            } else {
                                format!("Search: {}", self.library.read(cx).search)
                            },
                            &self.hover_title_epoch,
                        )
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.open_panel(Panel::Search, window, cx)
                        })),
                )
            });
        let header = if compact {
            div()
                .h(px(64.))
                .flex_shrink_0()
                .px_2()
                .mb_3()
                .border_b_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SIDEBAR))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    icon_size("back", IconName::ArrowLeft, 24.)
                        .w(px(48.))
                        .h(px(48.))
                        .accessibility_label("Back to playlists")
                        .hover_title("Back to playlists", &self.hover_title_epoch)
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.compact_playlist = false;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .id("playlist-heading")
                        .test_support()
                        .overflow_hidden()
                        .child(
                            heading(self.library.read(cx).name())
                                .truncate()
                                .hover_title(self.library.read(cx).name(), &self.hover_title_epoch),
                        )
                        .child(
                            muted(format!("{total} tracks"))
                                .id("playlist-track-count")
                                .test_support()
                                .whitespace_nowrap()
                                .text_xs()
                                .line_height(px(16.)),
                        ),
                )
                .child(actions)
                .into_any_element()
        } else {
            div().px_4().mb_4().child(actions).into_any_element()
        };
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .pt(px(if compact { 0. } else { 16. }))
            .when(compact || total > 0, |el| el.child(header));
        if total == 0 {
            return content
                .child(self.empty_playlist_view(cx))
                .into_any_element();
        }
        content
            .child(
                gpui_kit::base::Table::new("tracks-table")
                    .accessibility_label(format!("{} tracks", self.library.read(cx).name()))
                    .row_count(count + 1)
                    .column_count(if compact { 3 } else { 4 } + usize::from(self.selection_mode))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        gpui_kit::base::TableHeader::new("track-header")
                            .flex_shrink_0()
                            .child(
                                gpui_kit::base::TableRow::new("track-header-row", 0)
                                    .mx(px(if compact { 8. } else { 16. }))
                                    .px(px(if compact { 12. } else { 14. }))
                                    .mb_2()
                                    .flex()
                                    .items_center()
                                    .gap(px(if compact { 8. } else { 12. }))
                                    .h(px(if compact { 48. } else { 36. }))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .bg(rgb(SIDEBAR))
                                    .rounded(px(12.))
                                    .when(self.selection_mode, |el| {
                                        let entries = self.library.read(cx).entries();
                                        let all = !entries.is_empty()
                                            && entries.iter().all(|e| {
                                                self.library.read(cx).selection.contains(&e.source_index)
                                            });
                                        let toggle =
                                            cx.listener(move |app, _: &ClickEvent, _, cx| {
                                                if all {
                                                    app.library.clone().update(cx, |library, cx| { library.selection.clear(); cx.notify();  });
                                                } else {
                                                    app.library.clone().update(cx, |library, cx| { library.selection = library.entries()
                                                        .iter()
                                                        .map(|e| e.source_index)
                                                        .collect(); cx.notify();  });
                                                }
                                                cx.stop_propagation();
                                                cx.notify();
                                            });
                                        el.child(
                            div()
                                .w(px(if compact { 48. } else { 36. }))
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .child(
                                    gpui_kit::base::Checkbox::new("select-all-tracks")
                                        .checked(all)
                                        .indeterminate(!all && !self.library.read(cx).selection.is_empty())
                                        .accessibility_label("Select all tracks")
                                        .child(super::settings::check_indicator(
                                            if all {
                                                gpui_kit::base::CheckboxState::Checked
                                            } else if !self.library.read(cx).selection.is_empty() {
                                                gpui_kit::base::CheckboxState::Indeterminate
                                            } else {
                                                gpui_kit::base::CheckboxState::Unchecked
                                            },
                                            if compact { 24. } else { 16. },
                                        ))
                                        .on_change(move |_, event, window, cx| {
                                            toggle(event, window, cx)
                                        }),
                                ),
                        )
                                    })
                                    .child(
                                        gpui_kit::base::TableHead::new(
                                            "title-column",
                                            usize::from(self.selection_mode),
                                        )
                                        .flex_basis(px(0.))
                                        .flex_grow(if compact { 1. } else { 1.4 })
                                        .min_w_0()
                                        .flex()
                                        .child(
                                            gpui_kit::base::Button::new("sort-title")
                                                .accessibility_label("Sort by title")
                                                .flex()
                                                .justify_start()
                                                .items_center()
                                                .child("Title")
                                                .child(self.sort_icon("title", compact, cx))
                                                .gap_1()
                                                .text_color(rgb(MUTED))
                                                .flex_basis(px(0.))
                                                .flex_grow(if compact { 1. } else { 1.4 })
                                                .min_w_0()
                                                .on_click(cx.listener(
                                                    |app, event: &ClickEvent, _, cx| {
                                                        app.library.clone().update(cx, |library, cx| { library.toggle_sort(
                                                            "title",
                                                            event.modifiers().shift,
                                                        ); cx.notify();  });
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                    )
                                    .when(!compact, |el| {
                                        el.child(
                                            gpui_kit::base::TableHead::new(
                                                "artist-column",
                                                1 + usize::from(self.selection_mode),
                                            )
                                            .flex_basis(px(0.))
                                            .flex_grow(1.)
                                            .min_w_0()
                                            .flex()
                                            .child(
                                                gpui_kit::base::Button::new("sort-artist")
                                                    .accessibility_label("Sort by artist")
                                                    .flex()
                                                    .justify_start()
                                                    .items_center()
                                                    .child("Artist")
                                                    .child(self.sort_icon("artist", compact, cx))
                                                    .gap_1()
                                                    .text_color(rgb(MUTED))
                                                    .flex_basis(px(0.))
                                                    .flex_grow(1.)
                                                    .min_w_0()
                                                    .on_click(cx.listener(
                                                        |app, event: &ClickEvent, _, cx| {
                                                            app.library.clone().update(cx, |library, cx| { library.toggle_sort(
                                                                "artist",
                                                                event.modifiers().shift,
                                                            ); cx.notify();  });
                                                            cx.notify();
                                                        },
                                                    )),
                                            ),
                                        )
                                    })
                                    .child(
                                        gpui_kit::base::TableHead::new(
                                            "duration-column",
                                            if compact { 1 } else { 2 }
                                                + usize::from(self.selection_mode),
                                        )
                                        .w(px(if compact { 48. } else { 72. }))
                                        .flex_shrink_0()
                                        .flex()
                                        .child(
                                            gpui_kit::base::Button::new("sort-duration")
                                                .accessibility_label("Sort by duration")
                                                .flex()
                                                .items_center()
                                                .child("Time")
                                                .child(self.sort_icon("duration", compact, cx))
                                                .gap_1()
                                                .text_color(rgb(MUTED))
                                                .w(px(if compact { 48. } else { 72. }))
                                                .flex_shrink_0()
                                                .justify_end()
                                                .on_click(cx.listener(
                                                    |app, event: &ClickEvent, _, cx| {
                                                        app.library.clone().update(cx, |library, cx| { library.toggle_sort(
                                                            "duration",
                                                            event.modifiers().shift,
                                                        ); cx.notify();  });
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .w(px(if compact { 48. } else { 36. }))
                                            .flex_shrink_0(),
                                    ),
                            ),
                    )
                    .child(
                        gpui_kit::base::TableBody::new("track-rows")
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .child(if count == 0 {
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(muted(if total == 0 {
                                        "No tracks yet"
                                    } else {
                                        "No matching tracks"
                                    }))
                                    .into_any_element()
                            } else {
                                div()
                                    .px(px(if compact { 8. } else { 16. }))
                                    .flex_1()
                                    .min_h_0()
                                    .child(
                                        motion::Layer::scrolling(
                                            "library-tracks",
                                            count,
                                            window,
                                            cx,
                                            move |app,
                                                  range: std::ops::Range<usize>,
                                                  window,
                                                  cx| {
                                                range
                                                    .filter_map(|index| {
                                                        let entry = visible.get(index)?;
                                                        app.thumbnail(entry.track.id, cx);
                                                        let visible = visible.clone();
                                                        let style = div()
                                                            .w_full()
                                                            .h(px(if compact { 88. } else { 78. }))
                                                            .style()
                                                            .clone();
                                                        Some(
                                                            motion::Layer::cached(
                                                                ("library-track-view", index),
                                                                style,
                                                                &app.library,
                                                                (
                                                                    Arc::as_ptr(&visible) as *const () as usize,
                                                                    active_row == Some(entry.source_index),
                                                                    app.row_inputs(entry.track.id, cx),
                                                                ),
                                                                window,
                                                                cx,
                                                                move |app, _, cx| {
                                                                    let entry = &visible[index];
                                                                    app.track_row(
                                                                        entry,
                                                                        index,
                                                                        compact,
                                                                        active_row
                                                                            == Some(
                                                                                entry.source_index,
                                                                            ),
                                                                        cx,
                                                                    )
                                                                },
                                                            )
                                                            .into_any_element(),
                                                        )
                                                    })
                                                    .collect::<Vec<_>>()
                                            },
                                        )
                                        .size_full()
                                        .flex_1()
                                        .min_h_0(),
                                    )
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(in crate::ui) fn sort_icon(
        &self,
        column: &'static str,
        compact: bool,
        cx: &App,
    ) -> impl IntoElement {
        let direction = if self.library.read(cx).sort == column {
            Some(self.library.read(cx).descending)
        } else {
            self.library
                .read(cx)
                .additional_sort
                .iter()
                .find(|(key, _)| key == column)
                .map(|(_, desc)| *desc)
        };
        div()
            .id(SharedString::from(format!("sort-icon-{column}")))
            .test_support()
            .role(Role::Image)
            .aria_label(match direction {
                Some(false) => "Ascending",
                Some(true) => "Descending",
                None => "Unsorted",
            })
            .text_color(rgb(if direction.is_some() { FG } else { 0x606060 }))
            .child(glyph(
                match direction {
                    Some(false) => IconName::ArrowUp,
                    Some(true) => IconName::ArrowDown,
                    None => IconName::ArrowUpDown,
                },
                if compact { 24. } else { 14. },
            ))
    }

    pub(in crate::ui) fn bulk_actions_button(
        &self,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let count = self.library.read(cx).selection.len();
        Button::new("bulk-actions")
            .secondary()
            .accessibility_label(format!(
                "Actions for {count} selected {}",
                if count == 1 { "track" } else { "tracks" }
            ))
            .h(px(if compact { 48. } else { 32. }))
            .px_3()
            .gap_2()
            .when(!compact, |el| el.child("Actions"))
            .when(compact, |el| el.child(glyph(IconName::Ellipsis, 16.)))
            .child(
                div()
                    .px(px(6.))
                    .py(px(2.))
                    .rounded(px(6.))
                    .bg(tint(PRIMARY, 0x26))
                    .text_color(rgb(PRIMARY))
                    .text_size(px(11.))
                    .font_family(mono_font())
                    .child(count.to_string()),
            )
            .dropdown_menu(move |mut menu, window, cx| {
                menu = menu.min_w(px(208.)).max_w(px(256.)).group_label("Queue");
                for (label, action, icon) in [
                    ("Play next", TrackAction::Next, IconName::ListStart),
                    ("Add to end", TrackAction::Append, IconName::ListEnd),
                ] {
                    let view = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(icon)).on_click(
                        move |_, _, cx| {
                            view.update(cx, |app, cx| {
                                app.track_action(
                                    action.clone(),
                                    app.library.read(cx).selected_entries(),
                                    cx,
                                )
                            })
                        },
                    ));
                }
                let in_liked =
                    view.read(cx).library.read(cx).selected == PlaylistId::Builtin("liked".into());
                let target = view.clone();
                menu = menu.separator().group_label("Playlist").item(
                    PopupMenuItem::new(if in_liked {
                        "Remove from Liked"
                    } else {
                        "Add to Liked"
                    })
                    .icon(menu_icon(IconName::Heart))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |app, cx| {
                            app.track_action(
                                TrackAction::SetLiked(!in_liked),
                                app.library.read(cx).selected_entries(),
                                cx,
                            )
                        })
                    }),
                );
                let playlists = view
                    .read(cx)
                    .library
                    .read(cx)
                    .playlists
                    .as_ref()
                    .map(|p| p.custom.clone())
                    .unwrap_or_default();
                let target = view.clone();
                menu = menu.submenu_with_icon(
                    Some(menu_icon(IconName::ListPlus)),
                    "Add to playlist",
                    window,
                    cx,
                    move |mut menu, _, _| {
                        let create = target.clone();
                        menu = menu.item(
                            PopupMenuItem::new("Create playlist")
                                .icon(menu_icon(IconName::Plus))
                                .on_click(move |_, window, cx| {
                                    create.update(cx, |app, cx| {
                                        let tracks = app
                                            .library
                                            .read(cx)
                                            .selected_entries()
                                            .iter()
                                            .map(|e| e.track.id)
                                            .collect();
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
                            let id = playlist.id;
                            menu = menu.item(
                                PopupMenuItem::new(playlist.name.clone())
                                    .icon(menu_icon(IconName::ListPlus))
                                    .on_click(move |_, _, cx| {
                                        target.update(cx, |app, cx| {
                                            app.track_action(
                                                TrackAction::AddToPlaylist(id),
                                                app.library.read(cx).selected_entries(),
                                                cx,
                                            )
                                        })
                                    }),
                            );
                        }
                        menu
                    },
                );
                if matches!(
                    view.read(cx).library.read(cx).selected,
                    PlaylistId::Custom(_)
                ) {
                    let target = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new("Remove from playlist")
                            .icon(menu_icon(IconName::ListX))
                            .on_click(move |_, _, cx| {
                                target.update(cx, |app, cx| {
                                    app.track_action(
                                        TrackAction::Remove,
                                        app.library.read(cx).selected_entries(),
                                        cx,
                                    )
                                })
                            }),
                    );
                }
                menu = menu.group_label("Library");
                for (label, action, icon) in [
                    ("Cache", TrackAction::Cache, IconName::HardDriveDownload),
                    ("Download", TrackAction::Export, IconName::Download),
                ] {
                    let view = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(icon)).on_click(
                        move |_, _, cx| {
                            view.update(cx, |app, cx| {
                                app.track_action(
                                    action.clone(),
                                    app.library.read(cx).selected_entries(),
                                    cx,
                                )
                            })
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }

    pub(in crate::ui) fn filters_button(
        &self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        motion::Layer::bind(
            "filters-motion-layer",
            window,
            cx,
            move |app, window, cx| app.filters_button_layer(compact, window, cx),
        )
        .into_any_element()
    }

    pub(in crate::ui) fn filters_button_layer(
        &self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = "playlist-filters-popup";
        let anchor = Anchor::TopCenter;
        let offset = px(4.);
        let slide = px(-8.);
        let view = cx.entity();
        let content = view.clone();
        let size = if compact { 48. } else { 32. };
        let open = matches!(self.panel, Some(Panel::PlaylistFilters));
        let sample = motion::presence(id, open, false, window, cx);
        let popover = gpui_kit::base::Popover::new(id)
            .anchor(anchor)
            .offset(offset)
            .open(open)
            .trigger(
                icon_size(
                    "playlist-filters",
                    IconName::Funnel,
                    if compact { 24. } else { 16. },
                )
                .w(px(size))
                .h(px(size))
                .accessibility_label("Playlists filters")
                .hover_title("Playlists filters", &self.hover_title_epoch),
            )
            .on_open_change(move |open, window, cx| {
                view.update(cx, |app, cx| {
                    if *open {
                        app.open_panel(Panel::PlaylistFilters, window, cx)
                    } else if matches!(app.panel, Some(Panel::PlaylistFilters)) {
                        app.close_panel(window, cx)
                    }
                })
            })
            .content(move |_, window, cx| {
                content.update(cx, |app, cx| {
                    motion::popup_surface(app.filters_popover(window, cx), sample, slide)
                })
            });
        div()
            .relative()
            .child(popover)
            .when(!open && sample.progress > 0., |el| {
                el.child(motion::popup_exit(
                    id,
                    anchor,
                    offset,
                    motion::popup_surface(self.filters_popover(window, cx), sample, slide),
                ))
            })
            .into_any_element()
    }
}

impl PlayerApp {
    pub(in crate::ui) fn sidebar_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        motion::Layer::cached(
            "sidebar-view",
            div().size_full().style().clone(),
            &self.library,
            self.sidebar_inputs(cx),
            window,
            cx,
            Self::sidebar_view,
        )
        .into_any_element()
    }
    pub(in crate::ui) fn desktop_library_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use gpui_kit::component::resizable::{resize_handle, HandleEdge};
        let handle = resize_handle("sidebar-resize", Axis::Horizontal)
            .inside(HandleEdge::Trailing)
            .on_drag(SidebarResize, |drag, _, _, cx| cx.new(|_| *drag))
            .on_drag_move(
                cx.listener(|app, event: &DragMoveEvent<SidebarResize>, _, cx| {
                    app.preferences.sidebar_width =
                        f32::from(event.event.position.x).clamp(220., 480.);
                    app.save_preferences();
                    cx.notify();
                }),
            );
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .id("sidebar-panel")
                    .test_support()
                    .w(px(self.preferences.sidebar_width))
                    .min_w(px(220.))
                    .max_w(px(480.))
                    .h_full()
                    .relative()
                    .flex_shrink_0()
                    .child(self.sidebar_panel(window, cx))
                    .child(handle),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.library_view(false, window, cx)),
            )
            .into_any_element()
    }
}
