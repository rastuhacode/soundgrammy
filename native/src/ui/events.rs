use super::*;

impl PlayerApp {
    pub(super) fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        if self.user.is_none() && !matches!(event, Event::AuthRevoked) && !is_platform_event(&event)
        {
            return;
        }
        match event {
            #[cfg(target_os = "android")]
            Event::Back => self.pending_back = true,
            Event::AudioState(ref value) => {
                let previous_preferences = (
                    self.preferences.repeat,
                    self.preferences.shuffle,
                    self.preferences.shuffle_mode,
                );
                let snapshot = value;
                let Some(user) = self.user.as_ref() else {
                    return;
                };
                if snapshot.player.as_ref().is_some_and(|p| {
                    p.queue
                        .tracks
                        .iter()
                        .any(|t| t.tg_user_id != 0 && t.tg_user_id != user.id)
                }) {
                    return;
                }
                if snapshot.revision < self.playback.read(cx).revision {
                    return;
                }
                // Paused output may publish heartbeat revisions without any
                // visible change. Keep ordering without waking every view.
                let previous = &self.playback.read(cx);
                if snapshot.player.is_none()
                    && snapshot.error.is_none()
                    && previous.error.is_none()
                    && snapshot.last_control == previous.last_control
                    && snapshot.kind == previous.kind
                    && snapshot.status == previous.status
                    && snapshot.track_id == previous.track_id
                    && snapshot.attempt_id == previous.attempt_id
                    && snapshot.current_time_seconds == previous.current_time_seconds
                    && snapshot.duration_seconds == previous.duration_seconds
                    && snapshot.buffered_ranges == previous.buffered_ranges
                    && snapshot.volume_percent == previous.volume_percent
                    && snapshot.initial_loading == previous.initial_loading
                    && snapshot.seeking == previous.seeking
                {
                    self.playback
                        .update(cx, |state, _| state.revision = snapshot.revision);
                    return;
                }
                if snapshot.revision >= self.playback.read(cx).revision {
                    if let Some(error) = &snapshot.error {
                        if self
                            .playback
                            .read(cx)
                            .error
                            .as_ref()
                            .is_none_or(|old| old.message != error.message)
                        {
                            self.logs.push(error.message.clone());
                            self.error = Some(error.message.clone());
                        }
                    }
                    self.accept_snapshot(snapshot.clone(), cx);
                    self.preferences.repeat = self
                        .playback
                        .read(cx)
                        .player
                        .as_ref()
                        .map(|p| p.preferences.repeat)
                        .unwrap_or_default();
                    self.preferences.shuffle = self
                        .playback
                        .read(cx)
                        .player
                        .as_ref()
                        .map(|p| p.preferences.shuffle)
                        .unwrap_or_default();
                    self.preferences.shuffle_mode = self
                        .playback
                        .read(cx)
                        .player
                        .as_ref()
                        .map(|p| p.preferences.mode)
                        .unwrap_or_default();
                    if previous_preferences
                        != (
                            self.preferences.repeat,
                            self.preferences.shuffle,
                            self.preferences.shuffle_mode,
                        )
                    {
                        self.save_preferences();
                    }
                }
            }
            Event::AuthRevoked => self.reset(cx),
            Event::LibraryBatch(ref value) => {
                let batch = value;
                if self
                    .user
                    .as_ref()
                    .is_some_and(|u| u.id == batch.status.tg_user_id)
                    && self.library.clone().update(cx, |library, cx| {
                        let result = { library.apply_batch(batch) };
                        cx.notify();
                        result
                    })
                {
                    for id in &batch.removed_track_ids {
                        self.thumbs.remove(id);
                        self.download_progress.remove(id);
                        self.hq_thumbs.remove(id);
                    }
                }
            }
            Event::SyncStarted => self.syncing = true,
            Event::SyncFinished => {
                self.syncing = false;
                self.sync_progress = None;
            }
            Event::SyncFailed(ref value) => {
                self.syncing = false;
                self.sync_progress = None;
                let e = value;
                self.error = Some(e.message.clone());
            }
            Event::SyncProgress(ref value) => {
                let progress = value;
                self.sync_progress = Some((progress.done as i64, progress.total));
            }
            Event::DownloadProgress(ref value) => {
                let progress = value;
                let id = progress.track_id();
                if self.library.read(cx).tracks.iter().any(|t| t.id == id) {
                    if let Some(ratio) = progress.ratio() {
                        self.download_progress.insert(id, ratio);
                    } else {
                        self.download_progress.remove(&id);
                    }
                }
            }
            Event::CacheChanged(ref value) => {
                let changed = value;
                if changed.cleared {
                    self.library.clone().update(cx, |library, cx| {
                        library.cached.clear();
                        cx.notify();
                    });
                    self.download_progress.clear();
                }
                for id in &changed.track_ids {
                    self.download_progress.remove(id);
                    if changed.cached {
                        self.library.clone().update(cx, |library, cx| {
                            let result = { library.cached.insert(*id) };
                            cx.notify();
                            result
                        });
                    } else {
                        self.library.clone().update(cx, |library, cx| {
                            let result = { library.cached.remove(id) };
                            cx.notify();
                            result
                        });
                    }
                }
            }
            Event::ListenStats(ref value) => {
                let stats = value;
                self.library.clone().update(cx, |library, cx| {
                    let result = { library.stats.insert(stats.track_id, stats.clone()) };
                    cx.notify();
                    result
                });
                if matches!(&self.library.read(cx).selected, PlaylistId::Builtin(id) if id == "popular" || id == "recent")
                {
                    self.library.clone().update(cx, |library, cx| {
                        library.invalidate_track_list();
                        cx.notify();
                    });
                }
            }
            Event::LastFmStatus(ref value) => {
                let status = value;
                self.settings.update(cx, |feature, cx| {
                    feature.lastfm = Some(status.clone());
                    cx.notify();
                });
            }
            Event::ArtworkChanged(ref value) => {
                let art = value;
                if self.user.as_ref().is_some_and(|u| u.id == art.tg_user_id) {
                    self.accept_thumbnail(art.track_id, art.path.clone().into());
                }
            }
            Event::CacheProgress(ref value) => {
                let job = value;
                if !self.jobs.contains_key(&job.job_id) {
                    return;
                }
                self.jobs.insert(
                    job.job_id.clone(),
                    format!("Caching: {} / {}", job.current, job.total),
                );
            }
            Event::ExportProgress(ref value) => {
                let job = value;
                if !self.jobs.contains_key(&job.job_id) {
                    return;
                }
                self.jobs.insert(
                    job.job_id.clone(),
                    format!("Downloading: {} / {}", job.current, job.total),
                );
            }
        }
        cx.notify();
    }
}
