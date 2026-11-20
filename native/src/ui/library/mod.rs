use super::*;
mod actions;
pub(super) mod model;
mod row;
mod sidebar;
mod view;

impl PlayerApp {
    pub(super) fn hydrate(&mut self, cx: &mut Context<Self>, initial: bool) {
        let host = self.host();
        self.task(cx, model::hydrate(host), move |app, library, cx| {
            if !library.warnings.is_empty() {
                app.error = Some(library.warnings.join("\n"));
            }
            app.library.update(cx, |current, cx| {
                current.replace_hydration(library);
                cx.notify();
            });
            app.phase = Phase::Ready;
            if initial {
                let preferences = audio::session::Preferences {
                    repeat: app.preferences.repeat,
                    shuffle: app.preferences.shuffle,
                    mode: app.preferences.shuffle_mode,
                };
                app.command(
                    Command::Attach {
                        preferences: Some(preferences),
                    },
                    cx,
                );
                app.transport(audio::Control::Volume(app.preferences.volume), cx);
                app.reconnect(cx);
            }
        });
    }

    pub(super) fn sync(&mut self, full: bool, cx: &mut Context<Self>) {
        if self.syncing {
            self.pending_full_sync |= full;
            return;
        }
        self.syncing = true;
        self.sync_progress = None;
        let host = self.host();
        self.task(
            cx,
            async move {
                let state = host.state::<AppState>();
                Ok::<_, std::convert::Infallible>(
                    commands::sync_saved_music(
                        &state,
                        host,
                        Some(if full {
                            crate::telegram::saved_music::SyncMode::Full
                        } else {
                            crate::telegram::saved_music::SyncMode::Soft
                        }),
                    )
                    .await,
                )
            },
            |app, result, cx| {
                app.syncing = false;
                app.sync_progress = None;
                if let Err(error) = result {
                    app.logs.push(error.to_string());
                    app.error = Some(error.to_string());
                }
                app.hydrate(cx, false);
                if std::mem::take(&mut app.pending_full_sync) {
                    app.sync(true, cx);
                }
            },
        );
    }

    pub(super) fn set_statistics_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.update(cx, |feature, cx| {
            feature.statistics_enabled = enabled;
            cx.notify();
        });
        if !enabled
            && matches!(&self.library.read(cx).selected, PlaylistId::Builtin(id) if id=="popular" || id=="recent")
        {
            self.select_playlist(PlaylistId::All, cx);
        }
    }

    pub(super) fn select_playlist(&mut self, id: PlaylistId, cx: &mut Context<Self>) {
        if id != self.library.read(cx).selected {
            self.clear_track_search = true;
        }
        self.library.clone().update(cx, |library, cx| {
            library.select(id);
            cx.notify();
        });
        self.preferences.selected = self.library.read(cx).selected.key();
        self.compact_playlist = true;
        self.save_preferences();
        cx.notify();
    }

    pub(super) fn refresh_playlists(&mut self, cx: &mut Context<Self>) {
        let host = self.host();
        self.task(
            cx,
            async move { commands::playlists::list_playlists(&host.state::<AppState>()).await },
            |app, bundle, cx| {
                let old = app.library.read(cx).membership();
                app.library.clone().update(cx, |library, cx| {
                    library.playlists = Some(bundle);
                    cx.notify();
                });
                app.library.clone().update(cx, |library, cx| {
                    library.invalidate_track_list();
                    cx.notify();
                });
                if old != app.library.read(cx).membership() {
                    app.library.clone().update(cx, |library, cx| {
                        library.selection.clear();
                        cx.notify();
                    });
                }
            },
        );
    }

    pub(super) fn thumbnail(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.preview || self.thumbs.contains_key(&id) {
            return;
        }
        self.thumbs.insert(id, None);
        self.thumb_loading.insert(id);
        let host = self.host();
        self.quiet_task(
            cx,
            async move {
                let path = commands::get_track_thumbnail(
                    &host.state::<AppState>(),
                    host.clone(),
                    id,
                    Some(false),
                )
                .await
                .ok()
                .flatten();
                Ok::<_, crate::error::AppError>(path)
            },
            move |app, path, _| {
                app.thumb_loading.remove(&id);
                if app.thumbs.get(&id).is_none_or(Option::is_none) {
                    app.thumbs.insert(id, path.map(Into::into));
                    app.artwork_revision = app.artwork_revision.wrapping_add(1);
                }
            },
        );
    }

    pub(super) fn accept_thumbnail(&mut self, id: i64, path: std::path::PathBuf) {
        // The ID3 notification and Telegram request race. Quality can only rise.
        let current = self.thumbs.get(&id).and_then(Option::as_deref);
        if artwork::prefer_thumbnail(current, &path) {
            self.thumbs.insert(id, Some(path));
            self.artwork_revision = self.artwork_revision.wrapping_add(1);
        }
    }

    pub(super) fn high_quality_thumbnail(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.preview || !self.hq_thumbs.insert(id) {
            return;
        }
        let host = self.host();
        self.quiet_task(
            cx,
            async move {
                commands::get_track_thumbnail(
                    &host.state::<AppState>(),
                    host.clone(),
                    id,
                    Some(true),
                )
                .await
            },
            move |app, path, _| {
                // The normal thumbnail can finish while the high-quality
                // request is in flight. It is a fallback, never the winner.
                if let Some(path) = path {
                    app.accept_thumbnail(id, path.into());
                }
            },
        );
    }
}

impl PlayerApp {
    pub(super) fn row_inputs(&self, id: i64, cx: &App) -> impl PartialEq + Clone + 'static {
        (
            self.selection_mode,
            self.sort_drag,
            self.preferences.sidebar_width.to_bits(),
            self.playback
                .read(cx)
                .player
                .as_ref()
                .is_some_and(|p| p.is_playing),
            self.thumbs.get(&id).cloned(),
            self.download_progress.get(&id).map(|v| v.to_bits()),
        )
    }
    pub(super) fn sidebar_inputs(&self, cx: &App) -> impl PartialEq + Clone + 'static {
        (
            self.settings.read(cx).statistics_enabled,
            self.playlist_search.clone(),
            self.sort_drag,
            self.preferences.sidebar_width.to_bits(),
            self.preferences.hidden.clone(),
            self.preferences.playlist_sort.clone(),
            self.preferences.reversed,
            self.preferences.custom_order.clone(),
            self.artwork_revision,
            self.panel.as_ref().map(std::mem::discriminant),
        )
    }
}
