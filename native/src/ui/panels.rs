use super::views::*;
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    slider::Slider,
};

impl PlayerApp {
    pub(super) fn dismiss_hover_titles(&self) {
        self.hover_title_epoch
            .set(self.hover_title_epoch.get().wrapping_add(1));
    }

    pub(super) fn show_panel(&mut self, panel: Panel) {
        self.dismiss_hover_titles();
        if !matches!(
            panel,
            Panel::Queue | Panel::PlaylistFilters | Panel::CompactPlayer | Panel::Search
        ) {
            self.reset_volume_hover();
            self.seek_preview = None;
            self.seek_pointer = None;
        }
        if let Some(parent) = self.panel.take() {
            self.panel_stack.push(parent);
        }
        self.panel = Some(panel);
    }

    pub(super) fn close_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_hover_titles();
        if matches!(self.panel, Some(Panel::DownloadResult)) {
            self.download_results.pop_front();
            self.download_sections.clear();
            if !self.download_results.is_empty() {
                cx.notify();
                return;
            }
        }
        if matches!(self.panel, Some(Panel::Search)) {
            self.library.clone().update(cx, |library, cx| {
                library.search.clear();
                cx.notify();
            });
            self.inputs
                .search
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.panel = self.panel_stack.pop();
        cx.notify();
    }

    pub(super) fn open_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        self.touch_press = None;
        if matches!(panel, Panel::Settings) {
            self.settings.update(cx, |feature, cx| {
                feature.sections.clear();
                cx.notify();
            });
        }
        if matches!(panel, Panel::CompactPlayer) {
            if let Some(id) = self.playback.read(cx).track_id {
                self.hq_thumbs.remove(&id);
            }
        }
        if matches!(panel, Panel::SaveQueue) {
            self.queue_save_scope = QueueSaveScope::Full;
        }
        if matches!(panel, Panel::NewPlaylist) {
            self.new_playlist_tracks.clear();
            self.new_playlist_import = false;
        }
        if matches!(panel, Panel::Volume) {
            self.inputs.exact_volume.update(cx, |s, cx| {
                s.set_value(self.preferences.volume.round().to_string(), window, cx)
            });
        }
        self.error = None;
        match panel {
            Panel::NewPlaylist | Panel::SaveQueue => self
                .inputs
                .name
                .update(cx, |s, cx| s.set_value("", window, cx)),
            Panel::RenamePlaylist(id) => {
                let name = self
                    .library
                    .read(cx)
                    .playlists
                    .as_ref()
                    .and_then(|p| p.custom.iter().find(|p| p.id == id))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                self.inputs
                    .name
                    .update(cx, |s, cx| s.set_value(name, window, cx));
            }
            Panel::Settings | Panel::Proxy => self.load_settings(window, cx),
            _ => {}
        }
        let name_focus = matches!(
            panel,
            Panel::NewPlaylist | Panel::SaveQueue | Panel::RenamePlaylist(_)
        );
        self.show_panel(panel);
        if name_focus && self.panel_was_open {
            self.inputs.name.update(cx, |s, cx| s.focus(window, cx));
        }
        self.update_effects(window, cx);
        cx.notify();
    }

    pub(super) fn compact_player_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compact_open = matches!(self.panel, Some(Panel::CompactPlayer))
            || self
                .panel_stack
                .iter()
                .any(|p| matches!(p, Panel::CompactPlayer));
        let compact_motion =
            motion::presence("player-sheet-motion", compact_open, true, window, cx);
        if (compact_open || compact_motion.progress > 0.)
            && self.playback.read(cx).track_id.is_some()
        {
            self.compact_player_view(compact_motion, window, cx)
        } else {
            Empty.into_any_element()
        }
    }

    pub(super) fn modal_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Compact search is a non-modal page above the persistent player, just
        // like develop: no backdrop, dialog padding or exit overlay.
        if matches!(self.panel, Some(Panel::Search)) {
            self.retained_modal = None;
            return self.search_overlay(window, cx);
        }
        let active_modal = self.panel.as_ref().filter(|p| {
            !matches!(
                p,
                Panel::Queue | Panel::PlaylistFilters | Panel::CompactPlayer
            )
        });
        let docked = matches!(
            active_modal.or(self.retained_modal.as_ref()),
            Some(Panel::Navigation)
        );
        let modal = motion::presence("modal-motion", active_modal.is_some(), docked, window, cx);
        if let Some(panel) = active_modal {
            self.retained_modal = Some(panel.clone());
        } else if !modal.should_render() {
            self.retained_modal = None;
        }
        self.modal_progress = modal.progress;
        div()
            .absolute()
            .inset_0()
            .when_some(self.retained_modal.clone(), |el, panel| {
                el.child(self.panel_view(panel, window, cx))
            })
            .into_any_element()
    }

    pub(super) fn cancel_sort_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.sort_drag.is_none() {
            return false;
        }
        cx.stop_active_drag(window);
        self.sort_drag = None;
        cx.stop_propagation();
        cx.notify();
        true
    }

    pub(super) fn dismiss_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_fullscreen {
            self.pending_fullscreen = false;
            cx.notify();
        } else if self.volume_exact_open {
            self.volume_exact_open = false;
            self.focus.focus(window, cx);
        } else if self.panel.is_some() {
            self.close_panel(window, cx);
        } else if self.fullscreen {
            self.toggle_fullscreen(window, cx);
        } else if self.selection_mode {
            self.selection_mode = false;
            self.library.clone().update(cx, |library, cx| {
                library.clear_selection();
                cx.notify();
            });
        } else if self.compact_playlist {
            self.compact_playlist = false;
        } else {
            #[cfg(target_os = "android")]
            let _ = crate::android_keyring::background_activity();
        }
    }

    pub(super) fn search_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entries = self.library.read(cx).track_list().entries;
        let count = entries.len();
        let active_row = self
            .library
            .read(cx)
            .playing_source_index(self.playback.read(cx));
        let inset = if cfg!(target_os = "macos") && !window.is_fullscreen() {
            32.
        } else {
            0.
        };
        div()
            .id("compact-search-overlay")
            .test_support()
            .absolute()
            .top(px(inset))
            .left_0()
            .right_0()
            .bottom(px(if self.playback.read(cx).track_id.is_some() {
                96.
            } else {
                0.
            }))
            .bg(rgb(CARD))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        icon_size("close-track-search", IconName::ArrowLeft, 20.)
                            .hover_title("Close track search", &self.hover_title_epoch)
                            .on_click(
                                cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                            ),
                    )
                    .child(
                        Input::new(&self.inputs.search)
                            .cleanable(true)
                            .id("compact-search-input")
                            .aria_label("Search tracks")
                            .appearance(false)
                            .text_size(px(16.))
                            .flex_1()
                            .min_w_0(),
                    ),
            )
            .child(div().flex_shrink_0().px_5().py_3().child(muted(format!(
                "{count} {} in {}",
                if count == 1 { "track" } else { "tracks" },
                self.library.read(cx).name()
            ))))
            .child(if count == 0 {
                div()
                    .px_5()
                    .py_6()
                    .child(muted("No tracks found."))
                    .into_any_element()
            } else {
                motion::Layer::scrolling(
                    "search-results",
                    count,
                    window,
                    cx,
                    move |app, range: std::ops::Range<usize>, _, cx| {
                        range
                            .filter_map(|index| {
                                let entry = entries.get(index)?;
                                app.thumbnail(entry.track.id, cx);
                                Some(app.track_row(
                                    entry,
                                    index,
                                    true,
                                    active_row == Some(entry.source_index),
                                    cx,
                                ))
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .flex_1()
                .min_h_0()
                .w_full()
                .px_2()
                .into_any_element()
            })
            .into_any_element()
    }

    pub(super) fn panel_view(
        &mut self,
        panel: Panel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match panel {
            Panel::Navigation => return self.navigation_drawer(window, cx),
            Panel::Search => return self.search_overlay(window, cx),
            Panel::Queue => return div().into_any_element(),
            Panel::PlaylistFilters => return div().into_any_element(),
            _ => {}
        }
        if matches!(
            panel,
            Panel::NewPlaylist | Panel::RenamePlaylist(_) | Panel::SaveQueue
        ) {
            return self.playlist_dialog(&panel, window, cx);
        }
        if let Panel::TrackInfo(id) = panel {
            return self.track_info_dialog(id, window, cx);
        }
        if matches!(panel, Panel::Logs) {
            return self.logs_dialog(window, cx);
        }
        if matches!(panel, Panel::DownloadResult) {
            return self.download_dialog(window, cx);
        }
        let panel_title = match &panel {
            Panel::Navigation => "SoundGrammy",
            Panel::About => "About SoundGrammy",
            Panel::PlaylistFilters => "Playlists filters",
            Panel::Volume => "Volume",
            Panel::Search => "Search tracks",
            Panel::Settings => "Settings",
            Panel::Proxy => "Connection settings",
            Panel::Queue => "Queue",
            Panel::NewPlaylist => "New playlist",
            Panel::RenamePlaylist(_) => "Rename playlist",
            Panel::SaveQueue => "Save queue as playlist",
            Panel::TrackInfo(_) => "Track information",
            Panel::Logs => "Logs",
            Panel::DownloadResult => "Download finished",
            Panel::CompactPlayer => "Now playing",
            Panel::Confirm(_) => "Confirm",
            Panel::Import { .. } => "Import playlist",
        };
        let settings = matches!(panel, Panel::Settings);
        let content = match panel {
            Panel::Navigation
            | Panel::PlaylistFilters
            | Panel::Queue
            | Panel::NewPlaylist
            | Panel::RenamePlaylist(_)
            | Panel::SaveQueue
            | Panel::TrackInfo(_)
            | Panel::Logs
            | Panel::DownloadResult
            | Panel::CompactPlayer => unreachable!("specialized overlay handled above"),
            Panel::About => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(heading("SoundGrammy"))
                .child(muted(format!("Version {}", env!("CARGO_PKG_VERSION"))))
                .child("A local-first player for your Telegram profile music.")
                .child(muted("Copyright © 2026 Rasten Remizov · GPL-3.0-only"))
                .into_any_element(),
            Panel::Search => unreachable!("specialized search overlay handled above"),
            Panel::Volume => div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    button(
                        "mute-volume",
                        if self.preferences.volume == 0. {
                            "Unmute"
                        } else {
                            "Mute"
                        },
                    )
                    .on_click(cx.listener(|app, _, _, cx| app.mute(cx))),
                )
                .child(
                    Input::new(&self.inputs.exact_volume)
                        .aria_label("Exact volume percent")
                        .w(px(100.)),
                )
                .child(
                    Slider::new(&self.volume_slider)
                        .accessibility_label("Volume")
                        .flex_1(),
                )
                .into_any_element(),
            Panel::Settings => self.settings_view(window, cx),
            Panel::Proxy => self.proxy_view(cx),
            Panel::Confirm(action) => {
                let text=match action{Confirmation::Logout=>"Log out of your Telegram account?",Confirmation::FullReindex=>"Reindex your entire saved music library? Tracks will be hidden until indexing finishes.",Confirmation::ClearCache=>"Clear all cached audio?",Confirmation::ClearStatistics=>"Permanently clear local listening statistics?",Confirmation::DeletePlaylist(_)=>"Delete this playlist? Your Telegram music will be kept.",Confirmation::DisconnectLastFm=>"Disconnect Last.fm? Choose whether to retain or delete pending scrobbles."};
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(text)
                    .when(matches!(action, Confirmation::DisconnectLastFm), |el| {
                        el.child(
                            button(
                                "disconnect-delete",
                                "Delete pending scrobbles and disconnect",
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.panel = app.panel_stack.pop();
                                app.lastfm_action("disconnect-delete", cx);
                            })),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(button("cancel-confirm", "Cancel").on_click(
                                cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                            ))
                            .child(
                                Button::new("confirm")
                                    .primary()
                                    .label(if matches!(action, Confirmation::DisconnectLastFm) {
                                        "Retain and disconnect"
                                    } else {
                                        "Confirm"
                                    })
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.confirm(action.clone(), cx)
                                    })),
                            ),
                    )
                    .into_any_element()
            }
            Panel::Import { path, preview } => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(heading(preview.suggested_name.clone()))
                .child(
                    muted(
                        std::path::Path::new(&path)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.clone()),
                    )
                    .id("playlist-import-path")
                    .test_support()
                    .truncate()
                    .hover_title(path.clone(), &self.hover_title_epoch),
                )
                .child(Input::new(&self.inputs.name).aria_label("Imported playlist name"))
                .child(muted(format!(
                    "{} tracks available · {} unavailable",
                    preview.succeeded.len(),
                    preview.failed.len()
                )))
                .children(preview.failed.iter().map(|entry| {
                    muted(format!(
                        "{} — {}",
                        entry.title.as_deref().unwrap_or("Unknown title"),
                        entry.reason
                    ))
                }))
                .child(
                    Button::new("confirm-import")
                        .primary()
                        .label("Import playlist")
                        .on_click(cx.listener(move |app, _, _, cx| {
                            let path = path.clone();
                            let name = app.inputs.name.read(cx).value().trim().to_owned();
                            let name = (!name.is_empty()).then_some(name);
                            app.mutation(
                                cx,
                                move |host| async move {
                                    commands::playlists::import_playlist_json(
                                        &host.state::<AppState>(),
                                        path,
                                        name,
                                    )
                                    .await
                                },
                                |app, result, cx| {
                                    app.panel = None;
                                    app.notice = Some(format!(
                                        "Imported {} tracks into {} · {} unavailable",
                                        result.succeeded.len(),
                                        result.playlist_name,
                                        result.failed.len()
                                    ));
                                    app.refresh_playlists(cx);
                                },
                            );
                        })),
                )
                .into_any_element(),
        };
        let popup =
            div()
                .id("modal")
                .test_support()
                .track_focus(&self.modal_focus)
                .role(Role::Group)
                .aria_label(panel_title)
                .w(px(if settings { 512. } else { 448. }))
                .max_w_full()
                .max_h(px((f32::from(window.viewport_size().height)
                    - if cfg!(target_os = "macos") && !window.is_fullscreen() {
                        32.
                    } else {
                        0.
                    })
                    * 0.9))
                .flex()
                .flex_col()
                .rounded(px(16.))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(CARD))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .px_4()
                        .py_2()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(heading(panel_title).flex_1())
                        .child(icon("close-panel", IconName::X).on_click(
                            cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                        )),
                )
                .child(
                    div()
                        .id("panel-scroll")
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_4()
                        .child(content),
                )
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .into_any_element();
        self.modal_surface(popup, false, window, cx)
    }
}
