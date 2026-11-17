use super::super::*;
use crate::error::AppError;

impl PlayerApp {
    pub(in crate::ui) fn track_action(
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
                    _ => !ids.iter().all(|id| self.library.read(cx).liked(*id)),
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
                let playlist_id = match self.library.read(cx).selected {
                    PlaylistId::Custom(id) => id,
                    PlaylistId::Builtin(ref id) if id == "liked" => self
                        .library
                        .read(cx)
                        .playlists
                        .as_ref()
                        .map(|p| p.liked.id)
                        .unwrap_or(0),
                    _ => return,
                };
                let positions = entries.iter().map(|e| e.source_index as i64).collect();
                let expected = self.library.read(cx).membership();
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
                        app.library.clone().update(cx, |library, cx| {
                            library.clear_selection();
                            cx.notify();
                        });
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

    pub(in crate::ui) fn download_result(
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

    pub(in crate::ui) fn download_playlist(&mut self, cx: &mut Context<Self>) {
        let name = self.library.read(cx).name();
        let ids = self.library.read(cx).membership();
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

    pub(in crate::ui) fn save_playlist(&mut self, cx: &mut Context<Self>) {
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
                    self.playback
                        .read(cx)
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

    pub(in crate::ui) fn reorder_tracks(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if self.selection_mode
            || !self.library.read(cx).reorderable()
            || !self.library.read(cx).search.is_empty()
            || !self.library.read(cx).sort.is_empty()
        {
            return;
        }
        let expected = self.library.read(cx).membership();
        if from >= expected.len() || to >= expected.len() || from == to {
            return;
        }
        let mut ids = expected.clone();
        let id = ids.remove(from);
        ids.insert(to, id);
        let playlist_id = match self.library.read(cx).selected {
            PlaylistId::Custom(id) => id,
            _ => self
                .library
                .read(cx)
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

    pub(in crate::ui) fn import_playlist(&mut self, cx: &mut Context<Self>) {
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

    pub(in crate::ui) fn export_recipe(&mut self, playlist: PlaylistId, cx: &mut Context<Self>) {
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
            .read(cx)
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
}
