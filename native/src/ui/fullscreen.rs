use super::views::*;
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::Disableable;

impl PlayerApp {
    pub(super) fn fullscreen_controls_visible(&self, cx: &App) -> bool {
        cx.background_executor()
            .now()
            .saturating_duration_since(self.fullscreen_activity)
            < Duration::from_secs(3)
            || self.panel.is_some()
            || self.volume_hover
            || self.scrubbing.is_some()
    }

    pub(super) fn sync_fullscreen_cursor(&mut self, window: &Window, cx: &mut App) {
        let hidden = self.fullscreen
            && self.phase == Phase::Ready
            && window.is_window_active()
            && window.is_visible()
            && !self.fullscreen_controls_visible(cx);
        #[cfg(target_os = "macos")]
        {
            use gpui_kit::CursorHideMode;
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};

            // Let the fullscreen inactivity timer control the cursor while
            // GPUI's keyboard-driven hiding is suspended.
            let cursor_mode_changed = if self.fullscreen {
                if self.cursor_hide_mode_before_fullscreen.is_none() {
                    self.cursor_hide_mode_before_fullscreen = Some(cx.cursor_hide_mode());
                    cx.set_cursor_hide_mode(CursorHideMode::Never);
                    true
                } else {
                    false
                }
            } else if let Some(mode) = self.cursor_hide_mode_before_fullscreen.take() {
                cx.set_cursor_hide_mode(mode);
                true
            } else {
                false
            };
            if (cursor_mode_changed || self.fullscreen_cursor_hidden != hidden)
                && window.is_window_active()
                && objc2::MainThreadMarker::new().is_some()
                && HasWindowHandle::window_handle(window)
                    .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::AppKit(_)))
            {
                objc2_app_kit::NSCursor::setHiddenUntilMouseMoves(hidden);
            }
        }
        self.fullscreen_cursor_hidden = hidden;
    }

    pub(super) fn show_fullscreen_controls(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.fullscreen_activity = cx.background_executor().now();
        self.fullscreen_idle_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = view.update(cx, |_, cx| cx.notify());
        }));
        cx.notify();
    }

    pub(super) fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_hover_titles();
        self.reset_volume_hover();
        if self.pending_fullscreen {
            self.pending_fullscreen = false;
            return;
        }
        if !self.fullscreen && self.fullscreen_artwork_pending(cx) {
            self.pending_fullscreen = true;
            self.update_effects(window, cx);
            cx.notify();
            return;
        }
        self.fullscreen = !self.fullscreen;
        if self.fullscreen {
            // Reopening fullscreen retries a failed/deferred embedded-art probe.
            if let Some(id) = self.playback.read(cx).track_id {
                self.hq_thumbs.remove(&id);
            }
            self.show_fullscreen_controls(window, cx);
        } else {
            self.fullscreen_idle_task = None;
            self.fullscreen_visual
                .update(cx, |visual, _| visual.clear());
        }
        self.fullscreen_pointer = None;
        // AppKit snapshots the current window to animate its Space transition.
        // Let the new player/library scene paint first; otherwise that snapshot
        // can briefly restore the old scene after the fullscreen scene appears.
        self.fullscreen_request = self.fullscreen_request.wrapping_add(1);
        let request = self.fullscreen_request;
        let epoch = self.epoch;
        let target = self.fullscreen;
        let view = cx.weak_entity();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let _ = view.update(cx, |app, _| {
                    if app.epoch == epoch
                        && app.fullscreen_request == request
                        && window.is_fullscreen() != target
                    {
                        window.toggle_fullscreen();
                    }
                });
            });
        });
        self.focus.focus(window, cx);
        self.apply_display_awake(cx);
        self.update_effects(window, cx);
        cx.notify();
    }

    pub(super) fn apply_display_awake(&mut self, _cx: &mut Context<Self>) {
        if self.wake_requests.is_none() {
            let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<bool>();
            let host = self.host();
            crate::runtime::spawn(async move {
                while let Some(enabled) = receiver.recv().await {
                    let host = host.clone();
                    let result = crate::runtime::spawn_blocking(move || {
                        host.try_state::<crate::display_wake::DisplayWakeState>()
                            .map(|state| state.set_enabled(enabled))
                    })
                    .await;
                    if let Ok(Some(Err(error))) = result {
                        tracing::warn!(%error,"Display wake request failed");
                    }
                }
            });
            self.wake_requests = Some(sender);
        }
        let _ = self
            .wake_requests
            .as_ref()
            .unwrap()
            .send(self.fullscreen && self.preferences.keep_awake);
    }

    pub(super) fn fullscreen_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = self.palette_path.clone();
        let frame = artwork::FullscreenFrame {
            track_id: self.playback.read(cx).track_id,
            cover: path,
            image: self.fullscreen_cover.clone(),
            backdrop: self.fullscreen_backdrop.clone(),
            palette: self.palette,
            settings: self.preferences.bounce.clone(),
            profile: self.bounce_profile.clone(),
            profile_ready_at: self.bounce_ready_at,
            seconds: self.playback.read(cx).current_time_seconds,
            playing: self.playback.read(cx).status == audio::Status::Playing,
        };
        self.fullscreen_visual
            .update(cx, |visual, cx| visual.set_frame(frame, cx));
        let view = cx.weak_entity();
        let pointer_activity = canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let view = view.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        let _ = view.update(cx, |app, cx| {
                            if app.fullscreen && app.fullscreen_pointer != Some(event.position) {
                                app.fullscreen_pointer = Some(event.position);
                                app.show_fullscreen_controls(window, cx);
                            }
                        });
                    }
                });
            },
        )
        .absolute()
        .size_0();
        div()
            .id("fullscreen-player")
            .w_full()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .child(self.fullscreen_visual.clone())
            .child(pointer_activity)
            .child(motion::Layer::bind(
                "fullscreen-controls-layer",
                window,
                cx,
                Self::fullscreen_controls_layer,
            ))
            .into_any_element()
    }

    pub(super) fn fullscreen_controls_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let width = f32::from(window.viewport_size().width);
        let track = self
            .playback
            .read(cx)
            .player
            .as_ref()
            .and_then(|p| p.queue.current())
            .cloned();
        let controls_visible = self.fullscreen_controls_visible(cx);
        let controls =
            motion::fullscreen_controls("fullscreen-operations", controls_visible, 500, window, cx);
        let exit =
            motion::fullscreen_controls("fullscreen-exit", controls_visible, 150, window, cx);
        let subtitle = track
            .as_ref()
            .map(|track| {
                let mut value = artist(track);
                if let Some(player) = &self.playback.read(cx).player {
                    if let Some(source) = &player.queue.source {
                        value.push_str(&format!(" · {}", source.name));
                    }
                    if player.queue.tracks.len() > 1 {
                        value.push_str(&format!(
                            " · {} of {}",
                            player.queue.cursor + 1,
                            player.queue.tracks.len()
                        ));
                    }
                }
                value
            })
            .unwrap_or_default();
        div()
            .absolute()
            .inset_0()
            .when(exit.should_render(), |el| {
                el.child(
                    div()
                        .absolute()
                        .right_6()
                        .top_6()
                        .opacity(exit.progress)
                        .child(
                            icon_size("exit-fullscreen", IconName::Minimize2, 20.)
                                .rounded_full()
                                .border_1()
                                .border_color(rgba(0xffffff1a))
                                .w(px(40.))
                                .h(px(40.))
                                .accessibility_label("Exit fullscreen player")
                                .disabled(!controls_visible)
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.toggle_fullscreen(window, cx)
                                })),
                        ),
                )
            })
            .when(controls.should_render(), |el| {
                el.child(
                    div()
                        .absolute()
                        .id("fullscreen-controls-panel")
                        .test_support()
                        .bottom(px(-16. * (1. - controls.progress)))
                        .opacity(controls.progress)
                        .left_0()
                        .right_0()
                        .px_8()
                        .pb(px(28.))
                        .pt(px(96.))
                        .bg(linear_gradient(
                            180.,
                            linear_color_stop(rgba(0x00000000), 0.),
                            linear_color_stop(rgba(0x000000bf), 1.),
                        ))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .w_full()
                                .max_w(px(896.))
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .items_end()
                                        .justify_between()
                                        .gap_4()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .child(
                                                    div()
                                                        .id("fullscreen-title")
                                                        .test_support()
                                                        .hover_title(
                                                            track
                                                                .as_ref()
                                                                .map(title)
                                                                .unwrap_or_else(|| {
                                                                    "Unknown title".into()
                                                                }),
                                                            &self.hover_title_epoch,
                                                        )
                                                        .role(Role::Heading)
                                                        .aria_label(
                                                            track
                                                                .as_ref()
                                                                .map(title)
                                                                .unwrap_or_else(|| {
                                                                    "Unknown title".into()
                                                                }),
                                                        )
                                                        .truncate()
                                                        .text_2xl()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(
                                                            track
                                                                .as_ref()
                                                                .map(title)
                                                                .unwrap_or_else(|| {
                                                                    "Unknown title".into()
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    muted(subtitle.clone()).truncate().hover_title(
                                                        subtitle,
                                                        &self.hover_title_epoch,
                                                    ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(self.like_current(
                                                    "fullscreen-like",
                                                    32.,
                                                    20.,
                                                    cx,
                                                ))
                                                .child(self.queue_button(
                                                    "fullscreen-queue",
                                                    32.,
                                                    window,
                                                    cx,
                                                ))
                                                .child(self.volume_control(window, cx)),
                                        ),
                                )
                                .child(self.playback_progress(
                                    "fullscreen",
                                    track.is_none() || !controls_visible,
                                    window,
                                    cx,
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div().flex_1().child(muted(time(
                                                self.scrubbing
                                                    .map(|p| {
                                                        p / 100.
                                                            * self
                                                                .playback
                                                                .read(cx)
                                                                .duration_seconds
                                                    })
                                                    .unwrap_or(
                                                        self.playback.read(cx).current_time_seconds,
                                                    ),
                                            ))),
                                        )
                                        .child(self.playback_controls(false, false, width, cx))
                                        .child(div().flex_1().flex().justify_end().child(muted(
                                            time(self.playback.read(cx).duration_seconds),
                                        ))),
                                ),
                        )
                        .when(!controls_visible, |el| el.child(motion::exit_shield())),
                )
            })
            .into_any_element()
    }

    pub(super) fn load_bounce(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.bounce_track == Some(id) {
            return;
        }
        self.bounce_track = Some(id);
        self.bounce_profile = None;
        self.bounce_ready_at = None;
        let host = self.host();
        self.quiet_task(
            cx,
            async move {
                let state = host.state::<AppState>();
                commands::get_track_bounce_profile(&state, host, id).await
            },
            move |app, response, cx| {
                if app.bounce_track != Some(id) || app.playback.read(cx).track_id != Some(id) {
                    return;
                }
                if let crate::bounce_analysis::BounceProfileResponse::Ready {
                    frame_ms,
                    loudness_data,
                    onset_data,
                    bass_data,
                    mid_data,
                    treble_data,
                    ..
                } = response
                {
                    use base64::Engine;
                    let decoder = base64::engine::general_purpose::STANDARD;
                    if let (Ok(loudness), Ok(onset), Ok(bass), Ok(mid), Ok(treble)) = (
                        decoder.decode(loudness_data),
                        decoder.decode(onset_data),
                        decoder.decode(bass_data),
                        decoder.decode(mid_data),
                        decoder.decode(treble_data),
                    ) {
                        app.bounce_profile = artwork::BounceProfile::new(
                            frame_ms,
                            loudness,
                            onset,
                            [bass, mid, treble],
                        )
                        .map(Arc::new);
                        if app.bounce_profile.is_some() {
                            app.bounce_ready_at = Some(cx.background_executor().now());
                        }
                    }
                }
            },
        );
    }
}
