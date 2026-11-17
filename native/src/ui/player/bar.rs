use super::super::views::*;
use super::super::*;
use gpui_kit::assets::IconName;
use gpui_kit::base::slider::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    menu::{ContextMenuExt, PopupMenuItem},
    Disableable, Sizable,
};

impl PlayerApp {
    pub(in crate::ui) fn like_current(
        &self,
        id: &'static str,
        size: f32,
        symbol: f32,
        cx: &mut Context<Self>,
    ) -> Button {
        let current = self
            .playback
            .read(cx)
            .player
            .as_ref()
            .and_then(|p| p.queue.current());
        let liked = current.is_some_and(|t| self.library.read(cx).liked(t.id));
        let label = if liked {
            "Remove from liked"
        } else {
            "Add to liked"
        };
        Button::new(id)
            .ghost()
            .w(px(size))
            .h(px(size))
            .p_0()
            .accessibility_label(label)
            .hover_title(label, &self.hover_title_epoch)
            .child(if liked {
                filled_heart(symbol)
            } else {
                glyph(IconName::Heart, symbol)
            })
            .text_color(rgb(FG))
            .on_click(cx.listener(|app, _, _, cx| {
                if let Some(track) = app
                    .playback
                    .read(cx)
                    .player
                    .as_ref()
                    .and_then(|p| p.queue.current())
                    .cloned()
                {
                    app.track_action(
                        TrackAction::Like,
                        vec![Entry {
                            track,
                            source_index: 0,
                        }],
                        cx,
                    );
                }
            }))
    }

    pub(in crate::ui) fn queue_button(
        &mut self,
        id: &'static str,
        size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        motion::Layer::bind(
            (ElementId::from(id), "queue-motion-layer"),
            window,
            cx,
            move |app, window, cx| app.queue_button_layer(id, size, window, cx),
        )
        .into_any_element()
    }

    pub(in crate::ui) fn queue_button_layer(
        &mut self,
        id: &'static str,
        size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let content = view.clone();
        let drawer = id == "compact-player-queue";
        let anchor = if drawer {
            Anchor::TopRight
        } else {
            Anchor::BottomCenter
        };
        let offset = px(if drawer { 8. } else { 41. });
        let slide = px(if drawer { -8. } else { 8. });
        let open = matches!(self.panel, Some(Panel::Queue));
        let sample = motion::presence(id, open, false, window, cx);
        let popover = gpui_kit::base::Popover::new(id)
            .anchor(anchor)
            .offset(offset)
            .open(open)
            .trigger(
                icon_size(id, IconName::ListMusic, 20.)
                    .w(px(size))
                    .h(px(size))
                    .accessibility_label("Queue")
                    .hover_title("Queue", &self.hover_title_epoch),
            )
            .on_open_change(move |open, window, cx| {
                view.update(cx, |app, cx| {
                    if *open {
                        app.open_panel(Panel::Queue, window, cx)
                    } else if matches!(app.panel, Some(Panel::Queue)) {
                        app.close_panel(window, cx)
                    }
                })
            })
            .content(move |_, window, cx| {
                content.update(cx, |app, cx| {
                    motion::popup_surface(app.queue_popover(window, cx), sample, slide)
                })
            });
        div()
            .relative()
            .child(popover)
            .when(
                !open && sample.progress > 0. && self.playback.read(cx).track_id.is_some(),
                |el| {
                    el.child(motion::popup_exit(
                        id,
                        anchor,
                        offset,
                        motion::popup_surface(self.queue_popover(window, cx), sample, slide),
                    ))
                },
            )
            .into_any_element()
    }

    pub(in crate::ui) fn player_bar(
        &mut self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let width = f32::from(window.viewport_size().width);
        let session = self.playback.read(cx).player.clone();
        let player = session.as_ref();
        let track = player.and_then(|p| p.queue.current());
        let artwork_size = if compact {
            if width < 360. {
                48.
            } else {
                56.
            }
        } else {
            64.
        };
        let description = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap_3()
            .when_some(track, |el, track| {
                let art = div()
                    .id("player-artwork")
                    .test_support()
                    .size(px(artwork_size))
                    .flex_shrink_0()
                    .rounded(px(12.))
                    .bg(rgb(CARD))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(
                        self.thumbs.get(&track.id).cloned().flatten().is_none(),
                        |el| el.child(glyph(IconName::Music, 20.).text_color(rgb(MUTED))),
                    )
                    .when_some(self.thumbs.get(&track.id).cloned().flatten(), |el, path| {
                        el.child(
                            img(path)
                                .image_cache(&self.artwork_cache)
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .rounded(px(12.)),
                        )
                    });
                let art = if compact {
                    art.into_any_element()
                } else {
                    gpui_kit::base::Button::new("current-artwork")
                        .accessibility_label("Open fullscreen player")
                        .hover_title("Open fullscreen player", &self.hover_title_epoch)
                        .size(px(64.))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .relative()
                        .group("fullscreen-entry")
                        .child(art)
                        .child(
                            div()
                                .id("fullscreen-entry-icon")
                                .test_support()
                                .absolute()
                                .inset_0()
                                .rounded(px(12.))
                                .bg(rgba(0x00000073))
                                .flex()
                                .items_center()
                                .justify_center()
                                .opacity(0.)
                                .group_hover("fullscreen-entry", |el| el.opacity(1.))
                                .child(glyph(IconName::Maximize2, 20.)),
                        )
                        .on_click(
                            cx.listener(|app, _, window, cx| app.toggle_fullscreen(window, cx)),
                        )
                        .into_any_element()
                };
                el.child(art).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .child(
                            div()
                                .id("player-track-title")
                                .test_support()
                                .hover_title(title(track), &self.hover_title_epoch)
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .child(title(track)),
                        )
                        .child(
                            muted(artist(track))
                                .hover_title(artist(track), &self.hover_title_epoch)
                                .text_xs()
                                .line_height(px(16.))
                                .truncate(),
                        ),
                )
            });
        let description = if compact {
            gpui_kit::base::Button::new("current-track")
                .accessibility_label("Open player")
                .flex_1()
                .min_w_0()
                .cursor_pointer()
                .child(description)
                .on_click(cx.listener(|app, _, window, cx| {
                    app.open_panel(Panel::CompactPlayer, window, cx)
                }))
                .into_any_element()
        } else {
            div()
                .id("current-track")
                .test_support()
                .flex_1()
                .min_w_0()
                .child(description)
                .into_any_element()
        };
        let controls = self.playback_controls(compact, false, width, cx);
        let covered = matches!(self.panel, Some(Panel::CompactPlayer))
            || self
                .panel_stack
                .iter()
                .any(|panel| matches!(panel, Panel::CompactPlayer));
        div()
            .id("player-bar")
            .test_support()
            .w_full()
            .relative()
            .h(px(if compact { 80. } else { 96. }))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(CARD))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(self.playback_progress("bar", track.is_none() || covered, window, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(description)
                    .child(
                        div()
                            .when(!compact, |el| el.w(relative(0.333)))
                            .when(compact, |el| el.flex_shrink_0())
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(controls)
                            .when(!compact, |el| {
                                el.child(
                                    muted(format!(
                                        "{}  /  {}",
                                        time(
                                            self.scrubbing
                                                .map(|p| p / 100.
                                                    * self.playback.read(cx).duration_seconds)
                                                .unwrap_or(
                                                    self.playback.read(cx).current_time_seconds
                                                )
                                        ),
                                        time(self.playback.read(cx).duration_seconds)
                                    ))
                                    .font_family(mono_font())
                                    .text_xs(),
                                )
                            }),
                    )
                    .when(!compact, |el| {
                        el.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .child(self.queue_button("queue", 32., window, cx))
                                .child(self.like_current("like-current", 32., 20., cx))
                                .child(self.volume_control(window, cx)),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(in crate::ui) fn playback_controls(
        &self,
        compact: bool,
        drawer: bool,
        width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let small = compact || drawer;
        let control_size = if drawer {
            (width * 0.08).clamp(48., 64.)
        } else if compact {
            48.
        } else {
            32.
        };
        let primary_size = if drawer {
            (width * 0.1).clamp(64., 80.)
        } else if compact {
            if width < 360. {
                48.
            } else {
                56.
            }
        } else {
            44.
        };
        let symbol_size = if small { 24. } else { 20. };
        let session = self.playback.read(cx).player.clone();
        let player = session.as_ref();
        let track = player.and_then(|p| p.queue.current());
        let playing = player.is_some_and(|p| p.is_playing);
        let shuffle = player.is_some_and(|p| p.preferences.shuffle == audio::session::Shuffle::On);
        let repeat = player.map(|p| p.preferences.repeat).unwrap_or_default();
        let play_symbol = glyph(
            if playing {
                IconName::Pause
            } else {
                IconName::Play
            },
            if drawer { 32. } else { symbol_size },
        );
        let current_mode = player.map(|p| p.preferences.mode).unwrap_or_default();
        let view = cx.entity();
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(if drawer {
                (width * 0.02).clamp(8., 16.)
            } else if compact {
                4.
            } else {
                2.
            }))
            .when(!compact, |el| {
                el.child(
                    icon_size(
                        "repeat",
                        if repeat == audio::session::Repeat::One {
                            IconName::Repeat1
                        } else {
                            IconName::Repeat
                        },
                        symbol_size,
                    )
                    .w(px(control_size))
                    .h(px(control_size))
                    .text_color(rgb(MUTED))
                    .when(repeat != audio::session::Repeat::None, |el| {
                        el.text_color(rgb(PRIMARY))
                    })
                    .accessibility_label("Toggle repeat")
                    .hover_title("Toggle repeat", &self.hover_title_epoch)
                    .on_click(cx.listener(|app, _, _, cx| app.command(Command::ToggleRepeat, cx))),
                )
            })
            .child(
                icon_size("previous", IconName::SkipBack, symbol_size)
                    .accessibility_label("Previous track")
                    .hover_title("Previous track", &self.hover_title_epoch)
                    .w(px(control_size))
                    .h(px(control_size))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.command(Command::Previous { restart: true }, cx)
                    })),
            )
            .child(
                Button::new("play-pause")
                    .accessibility_label(if playing { "Pause" } else { "Play" })
                    .hover_title(
                        if playing { "Pause" } else { "Play" },
                        &self.hover_title_epoch,
                    )
                    .primary()
                    .rounded_full()
                    .mx(px(if small { 0. } else { 8. }))
                    .w(px(primary_size))
                    .h(px(primary_size))
                    .p_0()
                    .child(play_symbol)
                    .disabled(track.is_none())
                    .on_click(cx.listener(|app, _, _, cx| app.command(Command::Toggle, cx))),
            )
            .child(
                icon_size("next", IconName::SkipForward, symbol_size)
                    .accessibility_label("Next track")
                    .hover_title("Next track", &self.hover_title_epoch)
                    .w(px(control_size))
                    .h(px(control_size))
                    .on_click(cx.listener(|app, _, _, cx| app.command(Command::Next, cx))),
            )
            .when(!compact, |el| {
                el.child(
                    icon_size("shuffle", IconName::Shuffle, symbol_size)
                        .accessibility_label("Shuffle")
                        .hover_title(
                            format!(
                                "Shuffle: {}. Right-click to choose mode.",
                                if shuffle {
                                    match current_mode {
                                        audio::session::Mode::Random => "Random",
                                        audio::session::Mode::Variety => "Variety",
                                        audio::session::Mode::Rediscover => "Rediscover",
                                        audio::session::Mode::Smart => "Smart",
                                        audio::session::Mode::Fresh => "Fresh",
                                        audio::session::Mode::Duration => "Duration",
                                    }
                                } else {
                                    "Off"
                                }
                            ),
                            &self.hover_title_epoch,
                        )
                        .w(px(control_size))
                        .h(px(control_size))
                        .text_color(rgb(MUTED))
                        .when(shuffle, |el| el.text_color(rgb(PRIMARY)))
                        .on_click(
                            cx.listener(|app, _, _, cx| app.command(Command::ToggleShuffle, cx)),
                        )
                        .context_menu(move |mut menu, _, _| {
                            menu = menu.max_h(px(160.)).scrollable(true);
                            for (mode, label) in [
                                (audio::session::Mode::Random, "Random"),
                                (audio::session::Mode::Variety, "Variety"),
                                (audio::session::Mode::Rediscover, "Rediscover"),
                                (audio::session::Mode::Smart, "Smart"),
                                (audio::session::Mode::Fresh, "Fresh"),
                                (audio::session::Mode::Duration, "Duration"),
                            ] {
                                let view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(label)
                                        .checked(mode == current_mode)
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |app, cx| {
                                                app.command(Command::ShuffleMode { mode }, cx)
                                            })
                                        }),
                                );
                            }
                            menu
                        }),
                )
            })
            .into_any_element()
    }

    pub(in crate::ui) fn playback_progress(
        &self,
        location: &'static str,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        let disabled = disabled
            || !self.playback.read(cx).duration_seconds.is_finite()
            || self.playback.read(cx).duration_seconds <= 0.;
        let progress = self.seek_slider.read(cx).value().start();
        let progress = if progress.is_finite() {
            progress.clamp(0., 100.) / 100.
        } else {
            0.
        };
        let thumb_size = if self.scrubbing.is_some() { 15. } else { 12. };
        div()
            .id("playback-progress")
            .test_support()
            .on_prepaint(move |rect, _, _| measured.set(rect))
            .w_full()
            .relative()
            .h(px(4.))
            .child(
                div()
                    .id("seek-hit-area")
                    .absolute()
                    .top(px(-16.))
                    .left_0()
                    .right_0()
                    .h(px(32.))
                    // The invisible upper half crosses the library's last row.
                    // Only this slider should hover or handle a press there.
                    .when(!disabled, |el| el.block_mouse_except_scroll())
                    .on_hover(cx.listener(|app, hovered: &bool, _, cx| {
                        if !hovered {
                            app.seek_preview = None;
                            app.seek_pointer = None;
                            cx.notify();
                        }
                    }))
                    .on_mouse_move(cx.listener(move |app, event: &MouseMoveEvent, window, cx| {
                        if disabled || window.last_input_was_touch() {
                            app.seek_preview = None;
                            return;
                        }
                        let rect = bounds.get();
                        let fraction = ((f32::from(event.position.x - rect.origin.x))
                            / f32::from(rect.size.width).max(1.))
                        .clamp(0., 1.);
                        app.seek_preview = Some((
                            fraction,
                            fraction as f64 * app.playback.read(cx).duration_seconds,
                        ));
                        app.seek_pointer =
                            Some(point(event.position.x, rect.top() + px(32. / 3. - 16.)));
                        cx.notify();
                    }))
                    .when_some(
                        self.seek_preview.filter(|_| self.scrubbing.is_none()),
                        |el, (_, seconds)| {
                            el.child(
                                deferred(
                                    gpui_kit::base::TooltipPositioner::new(Bounds::new(
                                        self.seek_pointer.unwrap_or_default(),
                                        size(px(0.), px(0.)),
                                    ))
                                    .placement(gpui_kit::base::Placement::Top)
                                    .child(
                                        motion::Layer::bind(
                                            (ElementId::from(location), "seek-tooltip-layer"),
                                            window,
                                            cx,
                                            move |_, _, _| motion::seek_tooltip(time(seconds)),
                                        ),
                                    ),
                                )
                                .with_priority(gpui_kit::base::POPUP_PRIORITY),
                            )
                        },
                    )
                    .child(
                        BaseSlider::new(&self.seek_slider)
                            .accessibility_label("Seek")
                            .disabled(disabled)
                            .w_full()
                            .h(px(32.))
                            .child(
                                SliderTrack::new(&self.seek_slider)
                                    .disabled(disabled)
                                    .w_full()
                                    .h(px(32.))
                                    .relative()
                                    .when(!disabled, |track| {
                                        track.cursor_pointer().child(
                                            SliderIndicator::new(&self.seek_slider)
                                                .absolute()
                                                .size_full(),
                                        )
                                    })
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(16.))
                                            .h(px(4.))
                                            .w_full()
                                            .bg(rgb(0x404040)),
                                    )
                                    .when_some(
                                        self.loading_started.filter(|started| {
                                            cx.background_executor()
                                                .now()
                                                .saturating_duration_since(*started)
                                                >= Duration::from_millis(500)
                                        }),
                                        |el, started| {
                                            el.child(motion::Layer::bind(
                                                (ElementId::from(location), "seek-loading-layer"),
                                                window,
                                                cx,
                                                move |_, window, cx| {
                                                    Self::seek_loading_stripes(started, window, cx)
                                                },
                                            ))
                                        },
                                    )
                                    .children(self.playback.read(cx).buffered_ranges.iter().map(
                                        |range| {
                                            let duration =
                                                self.playback.read(cx).duration_seconds.max(1.);
                                            div()
                                                .absolute()
                                                .top(px(16.))
                                                .h(px(4.))
                                                .left(relative(
                                                    (range.start / duration).clamp(0., 1.) as f32,
                                                ))
                                                .w(relative(
                                                    ((range.end - range.start) / duration)
                                                        .clamp(0., 1.)
                                                        as f32,
                                                ))
                                                .bg(rgba(0xffffff4d))
                                        },
                                    ))
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(16.))
                                            .h(px(4.))
                                            .left_0()
                                            .id("seek-fill")
                                            .test_support()
                                            .w(relative(progress))
                                            .bg(rgb(PRIMARY)),
                                    )
                                    .child(
                                        SliderThumb::new(&self.seek_slider)
                                            .disabled(disabled)
                                            .absolute()
                                            .left(relative(progress))
                                            .top(px(18. - thumb_size / 2.))
                                            .ml(px(-thumb_size / 2.))
                                            .size(px(thumb_size))
                                            .rounded_full()
                                            .bg(rgb(PRIMARY))
                                            .opacity(
                                                if self.seek_preview.is_some()
                                                    || self.scrubbing.is_some()
                                                {
                                                    1.
                                                } else {
                                                    0.
                                                },
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(in crate::ui) fn seek_loading_stripes(
        started: std::time::Instant,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let offset = if window.is_visible() && !cx.reduce_motion() {
            window.request_animation_frame();
            (cx.background_executor()
                .now()
                .saturating_duration_since(started)
                .as_secs_f32()
                / 0.55
                * 8.49)
                % 8.49
        } else {
            0.
        };
        div()
            .id("seek-initial-loading")
            .test_support()
            .absolute()
            .top(px(16.))
            .h(px(4.))
            .w_full()
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, _, _| bounds,
                    move |_, bounds, window, _| {
                        window.paint_quad(fill(bounds, tint(PRIMARY, 0x1f)));
                        let mut stripes = PathBuilder::fill();
                        for index in -1..(f32::from(bounds.size.width) / 8.49).ceil() as i32 + 1 {
                            let x = px(index as f32 * 8.49 + offset);
                            stripes.move_to(bounds.origin + point(x, px(0.)));
                            stripes.line_to(bounds.origin + point(x + px(2.), px(0.)));
                            stripes.line_to(bounds.origin + point(x + px(6.), px(4.)));
                            stripes.line_to(bounds.origin + point(x + px(4.), px(4.)));
                            stripes.close();
                        }
                        if let Ok(path) = stripes.build() {
                            window.paint_path(path, tint(PRIMARY, 0x59));
                        }
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }

    pub(in crate::ui) fn reset_volume_hover(&mut self) {
        self.volume_hover = false;
        self.volume_exact_open = false;
        self.volume_pointer_over = [false; 2];
        self.volume_dragging = false;
        self.volume_close_task = None;
    }

    pub(in crate::ui) fn update_volume_hover(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.volume_dragging
            || self.volume_pointer_over.contains(&true)
            || self.volume_focus.contains_focused(window, cx)
        {
            self.volume_close_task = None;
            if !self.volume_hover {
                self.volume_hover = true;
                cx.notify();
            }
        } else if self.volume_hover && self.volume_close_task.is_none() {
            self.volume_close_task = Some(cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let _ = view.update_in(cx, |app, window, cx| {
                    app.volume_close_task = None;
                    if !app.volume_dragging
                        && !app.volume_pointer_over.contains(&true)
                        && !app.volume_focus.contains_focused(window, cx)
                    {
                        app.volume_hover = false;
                        app.volume_exact_open = false;
                        cx.notify();
                    }
                });
            }));
        }
    }

    pub(in crate::ui) fn volume_control(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        motion::Layer::bind("volume-motion-layer", window, cx, move |app, window, cx| {
            app.volume_control_layer(window, cx)
        })
        .into_any_element()
    }

    pub(in crate::ui) fn volume_control_layer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = "volume-hover-card";
        let behind_modal = self.retained_modal.is_some()
            || self.panel.as_ref().is_some_and(|panel| {
                !matches!(
                    panel,
                    Panel::Queue | Panel::PlaylistFilters | Panel::CompactPlayer | Panel::Search
                )
            });
        let sample = motion::presence(id, self.volume_hover, false, window, cx);
        let trigger = div()
            .id("volume-control")
            .relative()
            .on_hover(cx.listener(|app, hovered: &bool, window, cx| {
                app.volume_pointer_over[0] = *hovered;
                app.update_volume_hover(window, cx);
            }))
            .on_scroll(cx.listener(|app, event: &ScrollEvent, _, cx| {
                let (delta, lines) = match event.delta {
                    ScrollDelta::Lines(v) => (-(v.y as f64), true),
                    ScrollDelta::Pixels(v) => (-f32::from(v.y) as f64, false),
                };
                let (step, remainder) = volume_wheel(delta, lines, app.volume_wheel);
                app.volume_wheel = remainder;
                if step != 0 {
                    app.set_volume(app.preferences.volume + step as f64, cx);
                }
                cx.stop_propagation();
            }))
            .child(
                icon_size(
                    "volume",
                    if self.preferences.volume == 0. {
                        IconName::VolumeX
                    } else {
                        IconName::Volume2
                    },
                    20.,
                )
                .accessibility_label("Mute or unmute")
                .hover_title(
                    if self.preferences.volume == 0. {
                        "Unmute"
                    } else {
                        "Mute"
                    },
                    &self.hover_title_epoch,
                )
                .on_click(cx.listener(|app, _, window, cx| {
                    if window.last_input_was_touch() {
                        app.open_panel(Panel::Volume, window, cx);
                    } else {
                        app.mute(cx);
                    }
                })),
            );
        // HoverCard treats an active GPUI drag as loss of hover. Use the Base
        // Popup for the same anchor while the slider owns the press/release.
        gpui_kit::base::Popup::new(id, trigger)
            .anchor(Anchor::BottomCenter)
            .when(
                !behind_modal && (self.volume_hover || sample.progress > 0.),
                |popup| {
                    popup.content(
                        div()
                            .id("volume-surface")
                            .when(self.volume_hover, |el| {
                                el.on_hover(cx.listener(|app, hovered: &bool, window, cx| {
                                    app.volume_pointer_over[1] = *hovered;
                                    app.update_volume_hover(window, cx);
                                }))
                            })
                            .child(motion::popup_surface(self.volume_popup(cx), sample, px(4.))),
                    )
                },
            )
            .into_any_element()
    }

    pub(in crate::ui) fn finish_volume_drag(&mut self, window: &Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.volume_dragging) {
            self.update_volume_hover(window, cx);
            cx.notify();
        }
    }

    pub(in crate::ui) fn volume_popup(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            // Reserve room for three digits and the caret in both modes, so
            // entering an exact value does not resize the anchored popup.
            .w(px(48.))
            .id("volume-popover")
            .track_focus(&self.volume_focus)
            .test_support()
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .py_2()
            .bg(rgb(CARD))
            .border_1()
            .border_color(rgb(BORDER))
            .rounded(px(12.))
            .shadow_lg()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .when(!self.volume_exact_open, |el| {
                el.child(
                    button(
                        "exact-volume-chip",
                        format!("{}%", self.preferences.volume.round()),
                    )
                    .ghost()
                    .hover_title("Set exact volume", &self.hover_title_epoch)
                    .h(px(28.))
                    .px_1()
                    .text_xs()
                    .font_family(mono_font())
                    .text_color(rgb(MUTED))
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.volume_exact_open = true;
                        app.inputs.exact_volume.update(cx, |s, cx| {
                            s.set_value(app.preferences.volume.round().to_string(), window, cx);
                            s.focus(window, cx);
                        });
                        cx.notify();
                    })),
                )
            })
            .when(self.volume_exact_open, |el| {
                el.child(
                    Input::new(&self.inputs.exact_volume)
                        .aria_label("Exact volume percentage")
                        .small()
                        .appearance(false)
                        .w(px(36.))
                        .min_h(px(28.))
                        .px(px(2.))
                        .border_b_1()
                        .border_color(rgb(PRIMARY))
                        .text_xs()
                        .when(self.inputs.exact_volume.read(cx).value().is_empty(), |el| {
                            el.text_size(px(10.))
                        })
                        .font_family(mono_font())
                        .font_weight(FontWeight::NORMAL)
                        .text_center(),
                )
            })
            .child(
                BaseSlider::new(&self.volume_slider)
                    .accessibility_label("Volume")
                    .vertical()
                    .w(px(16.))
                    .h(px(96.))
                    .child(
                        SliderTrack::new(&self.volume_slider)
                            .axis(Axis::Vertical)
                            .capture_any_mouse_down(cx.listener(
                                |app, event: &MouseDownEvent, window, _| {
                                    if event.button == MouseButton::Left
                                        && !window.last_input_was_touch()
                                    {
                                        app.volume_dragging = true;
                                        app.volume_close_task = None;
                                    }
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|app, _, window, cx| {
                                    app.finish_volume_drag(window, cx);
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|app, _, window, cx| {
                                    app.finish_volume_drag(window, cx);
                                }),
                            )
                            .relative()
                            .size_full()
                            .child(
                                SliderIndicator::new(&self.volume_slider)
                                    .absolute()
                                    .size_full(),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px(6.))
                                    .w(px(4.))
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(SECONDARY)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px(6.))
                                    .bottom_0()
                                    .w(px(4.))
                                    .h(relative(self.preferences.volume as f32 / 100.))
                                    .rounded_full()
                                    .bg(rgb(PRIMARY)),
                            )
                            .child(
                                SliderThumb::new(&self.volume_slider)
                                    .axis(Axis::Vertical)
                                    .absolute()
                                    .left(px(2.))
                                    .bottom(relative(self.preferences.volume as f32 / 100.))
                                    .mb(px(-6.))
                                    .size(px(12.))
                                    .border_1()
                                    .border_color(rgb(PRIMARY))
                                    .rounded_full()
                                    .bg(rgb(0xffffff)),
                            ),
                    ),
            )
            .into_any_element()
    }
}
