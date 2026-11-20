use super::*;
mod bar;
mod queue;

impl PlayerApp {
    pub(super) fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        self.transport(command.into(), cx);
    }

    pub(super) fn transport(&mut self, control: audio::Control, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        if self.command_port.is_none() {
            self.command_port = Some(transport::CommandPort::new(
                self.host(),
                self.account_active.clone(),
            ));
        }
        let response = self.command_port.as_ref().unwrap().send(control);
        self.task(
            cx,
            async move {
                response
                    .await
                    .map_err(|_| "Playback operation cancelled".to_owned())?
            },
            |app, snapshot, cx| {
                if snapshot.revision >= app.playback.read(cx).revision {
                    app.accept_snapshot(snapshot, cx);
                }
            },
        );
    }

    pub(super) fn seek(&mut self, seconds: f64, cx: &mut Context<Self>) {
        if !seconds.is_finite() || !self.playback.read(cx).duration_seconds.is_finite() {
            return;
        }
        self.transport(
            audio::Control::SeekAttempt(
                seconds.clamp(0., self.playback.read(cx).duration_seconds.max(0.)),
                self.playback.read(cx).attempt_id.clone(),
            ),
            cx,
        );
    }

    pub(super) fn play_entries(
        &mut self,
        start: usize,
        shuffle: Option<audio::session::Shuffle>,
        cx: &mut Context<Self>,
    ) {
        let entries = self.library.read(cx).entries();
        if entries.is_empty() {
            return;
        }
        let tracks = entries.iter().map(|e| e.track.clone()).collect::<Vec<_>>();
        let source_indices = Some(entries.iter().map(|e| e.source_index).collect());
        let queue = Queue {
            source: Some(Source {
                r#type: SourceType::Playlist,
                playlist_id: match &self.library.read(cx).selected {
                    PlaylistId::Builtin(id) if id == "liked" => self
                        .library
                        .read(cx)
                        .playlists
                        .as_ref()
                        .map(|p| serde_json::json!(p.liked.id))
                        .unwrap_or_else(|| self.library.read(cx).selected.value()),
                    _ => self.library.read(cx).selected.value(),
                },
                name: self.library.read(cx).name(),
                track_ids: self.library.read(cx).membership(),
            }),
            tracks,
            cursor: start.min(entries.len() - 1) as i64,
            source_indices,
            base_entries: None,
        };
        self.command(
            Command::PlayPlaylist {
                queue,
                shuffle,
                toggle_if_current: true,
                pin_start: start > 0 || shuffle.is_none(),
            },
            cx,
        );
    }

    pub(super) fn accept_snapshot(
        &mut self,
        mut snapshot: audio::Snapshot,
        cx: &mut Context<Self>,
    ) {
        if snapshot.player.is_none() {
            snapshot.player = self.playback.read(cx).player.clone();
        }
        if snapshot
            .player
            .as_ref()
            .is_some_and(|p| p.queue.tracks.is_empty())
        {
            self.panel_stack
                .retain(|p| !matches!(p, Panel::Queue | Panel::CompactPlayer));
            if matches!(self.panel, Some(Panel::Queue | Panel::CompactPlayer)) {
                self.panel = self.panel_stack.pop();
            }
        }
        self.playback.update(cx, |state, cx| {
            *state = snapshot;
            cx.notify();
        });
        self.synced_seek_percent = None;
    }

    pub(super) fn set_volume(&mut self, value: f64, cx: &mut Context<Self>) {
        if !value.is_finite() {
            return;
        }
        self.preferences.volume = value.round().clamp(0., 100.);
        self.transport(audio::Control::Volume(self.preferences.volume), cx);
        self.save_preferences();
        cx.notify();
    }

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

impl PlayerApp {
    pub(super) fn player_bar_inputs(&self, cx: &App) -> impl PartialEq + Clone + 'static {
        let id = self.playback.read(cx).track_id;
        (
            (
                self.preferences.volume.to_bits(),
                self.volume_hover,
                self.volume_dragging,
                self.volume_exact_open,
                self.volume_pointer_over,
                self.scrubbing.map(f64::to_bits),
                self.seek_preview,
                self.seek_pointer,
            ),
            (
                self.panel.as_ref().map(std::mem::discriminant),
                self.retained_modal.as_ref().map(std::mem::discriminant),
                id.and_then(|id| self.thumbs.get(&id).cloned()),
                id.is_some_and(|id| self.library.read(cx).liked(id)),
                self.inputs.exact_volume.read(cx).value(),
                self.loading_started.is_some_and(|start| {
                    cx.background_executor()
                        .now()
                        .saturating_duration_since(start)
                        >= Duration::from_millis(500)
                }),
            ),
        )
    }
}
