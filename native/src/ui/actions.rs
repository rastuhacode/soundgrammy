use super::*;
impl PlayerApp {
    pub(super) fn confirm(&mut self, action: Confirmation, cx: &mut Context<Self>) {
        self.panel = self.panel_stack.pop();
        match action {
            Confirmation::Logout => {
                let host = self.host();
                self.reset(cx);
                self.task(
                    cx,
                    async move { commands::auth::logout(&host.state::<AppState>()).await },
                    |_, _, _| {},
                );
            }
            Confirmation::FullReindex => {
                // Persist full intent before network work, including when offline.
                let host = self.host();
                self.task(
                    cx,
                    async move { commands::prepare_full_reindex(&host.state::<AppState>()).await },
                    |app, batch, cx| {
                        app.library.clone().update(cx, |library, cx| {
                            let result = { library.apply_batch(&batch) };
                            cx.notify();
                            result
                        });
                        app.sync(true, cx);
                    },
                );
            }
            Confirmation::ClearCache => self.mutation(
                cx,
                |host| async move {
                    let state = host.state::<AppState>();
                    commands::clear_audio_cache(&state, host).await
                },
                |app, _, cx| {
                    app.notice = Some("Audio cache cleared".into());
                    app.hydrate(cx, false);
                },
            ),
            Confirmation::ClearStatistics => self.mutation(
                cx,
                |host| async move {
                    let state = host.state::<AppState>();
                    commands::listen_stats::clear_listen_statistics(&state, host).await
                },
                |app, _, cx| {
                    app.library.clone().update(cx, |library, cx| {
                        library.stats.clear();
                        cx.notify();
                    });
                    app.library.clone().update(cx, |library, cx| {
                        library.invalidate_track_list();
                        cx.notify();
                    });
                    app.hydrate(cx, false);
                },
            ),
            Confirmation::DeletePlaylist(id) => self.mutation(
                cx,
                move |host| async move {
                    let state = host.state::<AppState>();
                    commands::playlists::delete_playlist(&state, host, id).await
                },
                move |app, _, cx| {
                    if app.library.read(cx).selected == PlaylistId::Custom(id) {
                        app.library.clone().update(cx, |library, cx| {
                            library.select(PlaylistId::All);
                            cx.notify();
                        });
                        app.preferences.selected = "all".into();
                        app.clear_track_search = true;
                        app.save_preferences();
                    }
                    app.refresh_playlists(cx);
                },
            ),
            Confirmation::DisconnectLastFm => self.lastfm_action("disconnect", cx),
        }
    }
}
