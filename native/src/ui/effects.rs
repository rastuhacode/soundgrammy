//! Model-driven preparation. Network requests, timers, decoded artwork and
//! native fullscreen transitions run after intent/results, never during render.
use super::*;
impl PlayerApp {
    /// Some account/navigation intents have no Window in their callback. Queue
    /// reconciliation after that borrow ends, rather than waiting for a render
    /// or a remote operation to finish.
    pub(super) fn request_effects(&self, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = view.update(cx, |app, cx| app.update_effects(window, cx));
            });
        });
    }

    pub(super) fn update_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.updating_effects {
            return;
        }
        self.updating_effects = true;
        self.prepare_effects(window, cx);
        self.updating_effects = false;
    }
    fn prepare_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.playback.read(cx).initial_loading && !self.playback.read(cx).seeking {
            if self.loading_started.is_none() {
                self.loading_started = Some(cx.background_executor().now());
                self.loading_indicator_task = Some(cx.spawn(async move |view, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    let _ = view.update(cx, |_, cx| cx.notify());
                }));
            }
        } else {
            self.loading_started = None;
            self.loading_indicator_task = None;
        }
        if (self.phase == Phase::Login || self.playback.read(cx).track_id.is_none())
            && self.fullscreen
        {
            self.toggle_fullscreen(window, cx);
        }
        let session = self.playback.read(cx).player.clone();
        let player = session.as_ref();
        let current = player
            .as_ref()
            .and_then(|p| p.queue.current())
            .map(|t| t.id);
        if let Some(id) = current {
            if self.fullscreen
                || matches!(self.panel, Some(Panel::CompactPlayer))
                || self
                    .panel_stack
                    .iter()
                    .any(|panel| matches!(panel, Panel::CompactPlayer))
            {
                self.high_quality_thumbnail(id, cx);
            }
            self.thumbnail(id, cx);
        }
        let current_path = current.and_then(|id| self.thumbs.get(&id).cloned().flatten());
        self.prepare_fullscreen_artwork(current, current_path.clone(), window, cx);
        if self.pending_fullscreen && (self.phase != Phase::Ready || current.is_none()) {
            self.pending_fullscreen = false;
        }
        if self.pending_fullscreen && !self.fullscreen_artwork_pending(cx) {
            self.pending_fullscreen = false;
            self.toggle_fullscreen(window, cx);
        }
        if current_path != self.playing_artwork_path {
            self.playing_artwork_path = current_path.clone();
            artwork::release_texture(self.playing_artwork.take(), window, cx);
            if let Some(path) = current_path {
                let requested = path.clone();
                self.quiet_task(
                    cx,
                    async move {
                        Ok::<_, crate::error::AppError>(
                            crate::runtime::spawn_blocking(move || artwork::active_cover(&path))
                                .await
                                .ok()
                                .flatten(),
                        )
                    },
                    move |app, image, _| {
                        if app.playing_artwork_path.as_ref() == Some(&requested) {
                            app.playing_artwork = image;
                        }
                    },
                );
            }
        }
        if let Some(id) = current
            .filter(|_| self.fullscreen && self.preferences.bounce.enabled && !cx.reduce_motion())
        {
            self.load_bounce(id, cx);
        } else {
            self.bounce_track = None;
            self.bounce_profile = None;
            self.bounce_ready_at = None;
        }
        if self.phase == Phase::Ready {
            let cover_ids = self
                .library
                .read(cx)
                .playlists
                .as_ref()
                .map(|p| {
                    p.custom
                        .iter()
                        .flat_map(|p| p.track_ids.iter().take(4).copied())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for id in cover_ids {
                self.thumbnail(id, cx);
            }
        }
    }
}
