use super::*;
use crate::error::{AppError, AppResult};
impl PlayerApp {
    pub(super) fn set_volume(&mut self, value: f64, cx: &mut Context<Self>) {
        if !value.is_finite() {
            return;
        }
        self.preferences.volume = value.round().clamp(0., 100.);
        self.transport(audio::Control::Volume(self.preferences.volume), cx);
        self.save_preferences();
        cx.notify();
    }

    /// Serialize account mutations, and reject queued work after account changes.
    pub(super) fn mutation<T, F, Fut>(
        &mut self,
        cx: &mut Context<Self>,
        work: F,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        F: FnOnce(AppHost) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<T>> + Send + 'static,
    {
        let host = self.host();
        let generation = host.state::<AppState>().db.account_generation();
        let lock = self.mutations.clone();
        self.task(
            cx,
            async move {
                let _guard = lock.lock().await;
                let expected = generation?;
                if host.state::<AppState>().db.account_generation()? != expected {
                    return Err(AppError::NotAuthorized);
                }
                work(host).await
            },
            apply,
        );
    }
    pub(super) fn track_action(
        &mut self,
        action: TrackAction,
        entries: Vec<Entry>,
        cx: &mut Context<Self>,
    ) {
        if entries.is_empty() {
            return;
        }
        let ids = entries.iter().map(|e| e.track.id).collect::<Vec<_>>();
        match action {
            TrackAction::Next | TrackAction::Append => {
                self.command(
                    Command::Insert {
                        tracks: entries.into_iter().map(|e| e.track).collect(),
                        next: matches!(action, TrackAction::Next),
                    },
                    cx,
                );
            }
            TrackAction::Like | TrackAction::SetLiked(_) => {
                let desired = match action {
                    TrackAction::SetLiked(value) => value,
                    _ => !ids.iter().all(|id| self.library.liked(*id)),
                };
                self.mutation(
                    cx,
                    move |host| async move {
                        let state = host.state::<AppState>();
                        let mut failed = vec![];
                        for id in ids {
                            let bundle = commands::playlists::list_playlists(&state).await?;
                            if bundle.liked.track_ids.contains(&id) != desired
                                && commands::playlists::toggle_like(&state, host.clone(), id)
                                    .await
                                    .is_err()
                            {
                                failed.push(id);
                            }
                        }
                        Ok(failed)
                    },
                    |app, failed, cx| {
                        app.refresh_playlists(cx);
                        if !failed.is_empty() {
                            app.error = Some(format!("Could not update {} tracks.", failed.len()));
                        }
                    },
                );
            }
            TrackAction::AddToPlaylist(id) => self.mutation(
                cx,
                move |host| async move {
                    let state = host.state::<AppState>();
                    commands::playlists::add_tracks_to_playlist(&state, host, id, ids).await
                },
                |app, _, cx| {
                    app.refresh_playlists(cx);
                    app.notice = Some("Added to playlist".into());
                },
            ),
            TrackAction::Remove => {
                let playlist_id = match self.library.selected {
                    PlaylistId::Custom(id) => id,
                    PlaylistId::Builtin(ref id) if id == "liked" => self
                        .library
                        .playlists
                        .as_ref()
                        .map(|p| p.liked.id)
                        .unwrap_or(0),
                    _ => return,
                };
                let positions = entries.iter().map(|e| e.source_index as i64).collect();
                let expected = self.library.membership();
                self.mutation(
                    cx,
                    move |host| async move {
                        let state = host.state::<AppState>();
                        commands::playlists::remove_tracks_from_playlist(
                            &state,
                            host,
                            playlist_id,
                            positions,
                            expected,
                        )
                        .await
                    },
                    |app, _, cx| {
                        app.library.clear_selection();
                        app.refresh_playlists(cx);
                    },
                );
            }
            TrackAction::Cache => {
                let job = format!("cache-{}", crate::time::now_ms());
                self.jobs.insert(job.clone(), "Preparing cache…".into());
                let host = self.host();
                let result_job = job.clone();
                self.task(
                    cx,
                    async move {
                        let state = host.state::<AppState>();
                        Ok::<_, std::convert::Infallible>(
                            commands::cache_tracks(&state, host, ids, Some(job)).await,
                        )
                    },
                    move |app, result, _| {
                        app.jobs.remove(&result_job);
                        let result = match result {
                            Ok(result) => result,
                            Err(error) => {
                                app.logs.push(error.to_string());
                                app.error = Some(error.to_string());
                                return;
                            }
                        };
                        app.notice = Some(format!(
                            "Cached {} tracks; {} failed",
                            result.succeeded.len(),
                            result.failed.len()
                        ));
                        if !result.failed.is_empty() {
                            app.logs
                                .extend(result.failed.iter().map(|f| f.error.clone()));
                        }
                    },
                );
            }
            TrackAction::RemoveCache => self.mutation(
                cx,
                move |host| async move {
                    let state = host.state::<AppState>();
                    for id in ids {
                        commands::remove_track_from_cache(&state, host.clone(), id).await?;
                    }
                    Ok(())
                },
                |_, _, _| {},
            ),
            TrackAction::Export => {
                let host = self.host();
                self.task(
                    cx,
                    async move {
                        let state = host.state::<AppState>();
                        commands::export_tracks(&state, host, ids).await
                    },
                    |app, result, cx| app.download_result(result, cx),
                );
            }
            TrackAction::Info => {
                let id = ids[0];
                self.metadata = None;
                self.show_panel(Panel::TrackInfo(id));
                let host = self.host();
                self.quiet_task(
                    cx,
                    async move { commands::track_metadata(&host.state::<AppState>(), id).await },
                    move |app, metadata, _| {
                        if matches!(app.panel,Some(Panel::TrackInfo(current)) if current==id) {
                            app.metadata = Some((id, metadata));
                        }
                    },
                );
            }
        }
        cx.notify();
    }
    pub(super) fn download_result(
        &mut self,
        result: crate::export::PlaylistDownloadResult,
        cx: &mut Context<Self>,
    ) {
        self.logs.extend(
            result
                .failed
                .iter()
                .map(|f| format!("{}: {}", f.title.as_deref().unwrap_or("Track"), f.error)),
        );
        self.logs.extend(result.warnings.clone());
        #[cfg(not(target_os = "android"))]
        if let Some(folder) = &result.folder_path {
            cx.reveal_path(std::path::Path::new(folder));
        }
        self.download_results.push_back(result);
        if !matches!(self.panel, Some(Panel::DownloadResult)) {
            self.download_sections.clear();
            self.show_panel(Panel::DownloadResult);
        }
        cx.notify();
    }
    pub(super) fn download_playlist(&mut self, cx: &mut Context<Self>) {
        let name = self.library.name();
        let ids = self.library.membership();
        if ids.is_empty() {
            return;
        }
        let job = format!("download-{}", crate::time::now_ms());
        self.jobs.insert(job.clone(), "Preparing download…".into());
        let result_job = job.clone();
        let host = self.host();
        self.task(
            cx,
            async move {
                let state = host.state::<AppState>();
                Ok::<_, std::convert::Infallible>(
                    commands::download_playlist(&state, host, name, ids, job).await,
                )
            },
            move |app, result, cx| {
                app.jobs.remove(&result_job);
                match result {
                    Ok(result) => app.download_result(result, cx),
                    Err(error) => {
                        app.logs.push(error.to_string());
                        app.error = Some(error.to_string());
                    }
                }
            },
        );
    }
    pub(super) fn save_playlist(&mut self, cx: &mut Context<Self>) {
        let name = self.inputs.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.error = Some("Playlist name is required".into());
            cx.notify();
            return;
        }
        if name.chars().count() > 100 {
            self.error = Some("Playlist name must be at most 100 characters".into());
            cx.notify();
            return;
        }
        match self.panel {
            Some(Panel::RenamePlaylist(id)) => {
                self.mutation(
                    cx,
                    move |host| async move {
                        let state = host.state::<AppState>();
                        commands::playlists::update_playlist(&state, host, id, Some(name)).await
                    },
                    |app, _, cx| {
                        app.panel = None;
                        app.refresh_playlists(cx);
                    },
                );
            }
            _ => {
                let ids = if matches!(self.panel, Some(Panel::SaveQueue)) {
                    self.snapshot
                        .player
                        .as_ref()
                        .map(|p| queue_save_ids(&p.queue, self.queue_save_scope))
                        .unwrap_or_default()
                } else {
                    self.new_playlist_tracks.clone()
                };
                if matches!(self.panel, Some(Panel::SaveQueue)) && ids.is_empty() {
                    self.error = Some("Nothing to save for this scope".into());
                    cx.notify();
                    return;
                }
                self.mutation(
                    cx,
                    move |host| async move {
                        commands::playlists::create_playlist(
                            &host.state::<AppState>(),
                            name,
                            Some(ids),
                        )
                        .await
                    },
                    |app, playlist, cx| {
                        app.panel = None;
                        app.panel_stack.clear();
                        app.select_playlist(PlaylistId::Custom(playlist.id), cx);
                        app.refresh_playlists(cx);
                    },
                );
            }
        }
    }
    pub(super) fn reorder_tracks(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if self.selection_mode
            || !self.library.reorderable()
            || !self.library.search.is_empty()
            || !self.library.sort.is_empty()
        {
            return;
        }
        let expected = self.library.membership();
        if from >= expected.len() || to >= expected.len() || from == to {
            return;
        }
        let mut ids = expected.clone();
        let id = ids.remove(from);
        ids.insert(to, id);
        let playlist_id = match self.library.selected {
            PlaylistId::Custom(id) => id,
            _ => self
                .library
                .playlists
                .as_ref()
                .map(|p| p.liked.id)
                .unwrap_or(0),
        };
        self.mutation(
            cx,
            move |host| async move {
                let state = host.state::<AppState>();
                commands::playlists::reorder_playlist_tracks(
                    &state,
                    host,
                    playlist_id,
                    ids,
                    expected,
                    Some([from as i64, to as i64]),
                )
                .await
            },
            |app, _, cx| app.refresh_playlists(cx),
        );
    }
    pub(super) fn import_playlist(&mut self, cx: &mut Context<Self>) {
        self.task(
            cx,
            async {
                let path = crate::runtime::spawn_blocking(file_dialog::open_json)
                    .await
                    .map_err(|_| AppError::msg("File dialog failed"))??;
                Ok::<_, AppError>(path)
            },
            |app, path, cx| {
                if let Some(path) = path {
                    let path = path.to_string_lossy().into_owned();
                    let host = app.host();
                    let output_path = path.clone();
                    app.task(
                        cx,
                        async move {
                            commands::playlists::analyze_playlist_json(
                                &host.state::<AppState>(),
                                path,
                            )
                            .await
                        },
                        move |app, preview, _| {
                            app.panel = Some(Panel::Import {
                                path: output_path,
                                preview,
                            });
                        },
                    );
                }
            },
        );
    }
    pub(super) fn export_recipe(&mut self, playlist: PlaylistId, cx: &mut Context<Self>) {
        let source = match playlist {
            PlaylistId::Custom(id) => {
                crate::playlist_recipe::PlaylistRecipeSource::Custom { playlist_id: id }
            }
            PlaylistId::Builtin(ref id) if id == "liked" => {
                crate::playlist_recipe::PlaylistRecipeSource::Liked
            }
            _ => return,
        };
        let name = self
            .library
            .items(&self.preferences)
            .into_iter()
            .find(|p| p.id == playlist)
            .map(|p| p.name)
            .unwrap_or_else(|| "Playlist".into());
        let host = self.host();
        self.task(
            cx,
            async move {
                let path = crate::runtime::spawn_blocking(move || file_dialog::save_json(name))
                    .await
                    .map_err(|_| AppError::msg("File dialog failed"))??;
                if let Some(path) = path {
                    commands::playlists::export_playlist_json(
                        &host.state::<AppState>(),
                        source,
                        path.path.to_string_lossy().into_owned(),
                    )
                    .await?;
                    path.commit()?;
                    return Ok::<_, AppError>(true);
                }
                Ok::<_, AppError>(false)
            },
            |app, exported, _| {
                if exported {
                    app.notice = Some("Playlist recipe exported".into());
                }
            },
        );
    }
    pub(super) fn confirm(&mut self, action: Confirmation, cx: &mut Context<Self>) {
        self.panel = self.panel_stack.pop();
        match action {
            Confirmation::Logout => {
                let host = self.host();
                self.reset();
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
                        app.library.apply_batch(&batch);
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
                    app.library.stats.clear();
                    app.library.invalidate_track_list();
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
                    if app.library.selected == PlaylistId::Custom(id) {
                        app.library.select(PlaylistId::All);
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
    pub(super) fn save_cache_settings(&mut self, cx: &mut Context<Self>) {
        let limit = finite_nonnegative(&self.inputs.cache_limit.read(cx).value(), "Size limit");
        let ttl = finite_nonnegative(&self.inputs.cache_ttl.read(cx).value(), "Keep-for days");
        let (limit, ttl) = match (limit, ttl) {
            (Ok(l), Ok(t)) => (l, t),
            (Err(e), _) | (_, Err(e)) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        self.mutation(
            cx,
            move |host| async move {
                let state = host.state::<AppState>();
                let settings = commands::set_cache_settings(
                    &state,
                    host,
                    Some((limit * 1024_f64.powi(3)).round() as i64),
                    Some((ttl * 86400.).round() as i64),
                )
                .await?;
                let usage = commands::get_cache_usage(&state).await?;
                Ok((settings, usage))
            },
            |app, (settings, usage), _| {
                app.cache_settings = Some(settings);
                app.cache_usage = Some(usage);
                app.notice = Some("Cache settings saved".into());
            },
        );
    }
    pub(super) fn save_proxy(&mut self, cx: &mut Context<Self>) {
        let settings = crate::proxy_settings::ProxySettings {
            enabled: self.proxy.enabled,
            server: self.inputs.proxy_server.read(cx).value().to_string(),
            port: match self.inputs.proxy_port.read(cx).value().parse::<u16>() {
                Ok(p) => p,
                Err(_) if !self.proxy.enabled => 0,
                Err(_) => {
                    self.error = Some("Enter a port between 1 and 65535".into());
                    return;
                }
            },
            secret: self.inputs.proxy_secret.read(cx).value().to_string(),
        };
        if let Err(e) = settings.validate() {
            self.error = Some(e.to_string());
            return;
        }
        self.mutation(
            cx,
            move |host| async move {
                commands::set_proxy_settings(
                    &host.state::<AppState>(),
                    settings.enabled,
                    settings.server,
                    settings.port,
                    settings.secret,
                )
                .await
            },
            |app, _, cx| {
                app.panel = app.panel_stack.pop();
                app.online = false;
                if app.phase == Phase::Ready {
                    app.sync(false, cx);
                }
            },
        );
    }
    pub(super) fn parse_proxy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match crate::proxy_settings::from_proxy_link(&self.inputs.proxy_link.read(cx).value()) {
            Ok(settings) => {
                self.inputs
                    .proxy_server
                    .update(cx, |s, cx| s.set_value(settings.server.clone(), window, cx));
                self.inputs.proxy_port.update(cx, |s, cx| {
                    s.set_value(settings.port.to_string(), window, cx)
                });
                self.inputs
                    .proxy_secret
                    .update(cx, |s, cx| s.set_value(settings.secret.clone(), window, cx));
                self.proxy = settings;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub(super) fn lastfm_action(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let enabled = !self.lastfm.as_ref().is_some_and(|s| s.enabled);
        self.mutation(
            cx,
            move |host| async move {
                let state = host.state::<AppState>();
                match action {
                    "connect" => commands::lastfm::start_lastfm_auth(&state, host).await,
                    "complete" => commands::lastfm::complete_lastfm_auth(&state, host).await,
                    "cancel" => commands::lastfm::cancel_lastfm_auth(&state, host).await,
                    "disconnect" => {
                        commands::lastfm::disconnect_lastfm(&state, host, Some("retain".into()))
                            .await
                    }
                    "disconnect-delete" => {
                        commands::lastfm::disconnect_lastfm(&state, host, Some("delete".into()))
                            .await
                    }
                    "toggle" => commands::lastfm::set_lastfm_enabled(&state, host, enabled).await,
                    "profile" => {
                        commands::lastfm::open_lastfm_profile(&state, host).await?;
                        commands::lastfm::get_lastfm_status(&state).await
                    }
                    _ => {
                        commands::lastfm::flush_lastfm_queue(&state).await?;
                        commands::lastfm::get_lastfm_status(&state).await
                    }
                }
            },
            |app, status, _| app.lastfm = Some(status),
        );
    }
}

impl PlayerApp {
    pub(super) fn mute(&mut self, cx: &mut Context<Self>) {
        let volume = if self.preferences.volume > 0. {
            self.previous_volume = self.preferences.volume;
            0.
        } else {
            self.previous_volume.max(1.)
        };
        self.preferences.volume = volume;
        self.save_preferences();
        self.transport(audio::Control::Volume(volume), cx);
    }
}
