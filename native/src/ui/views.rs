//! App shell layout and small shared presentation elements. Feature rendering
//! lives in library/, player/, fullscreen, panels and login.
pub(super) use super::theme::{BG, BORDER, CARD, FG, MUTED, PRIMARY, SIDEBAR};
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    TitleBar,
};
#[derive(Clone)]
pub(super) struct TrackDrag {
    pub(super) index: usize,
    pub(super) playlist: PlaylistId,
}
#[derive(Clone)]
pub(super) struct QueueDrag {
    pub(super) index: usize,
    pub(super) revision: u64,
}
#[derive(Clone)]
pub(super) struct PlaylistDrag(pub(super) PlaylistId);
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SortList {
    Tracks,
    Queue,
    Playlists,
}
#[derive(Clone, Copy, PartialEq)]
pub(super) struct SortDrag {
    pub list: SortList,
    pub from: usize,
    pub over: usize,
}
pub(super) fn drag_shift(drag: Option<SortDrag>, list: SortList, index: usize, stride: f32) -> f32 {
    match drag.filter(|d| d.list == list) {
        Some(d) if d.from < d.over && index > d.from && index <= d.over => -stride,
        Some(d) if d.from > d.over && index >= d.over && index < d.from => stride,
        _ => 0.,
    }
}
pub(super) struct DragPreview {
    pub(super) label: String,
    pub(super) artist: Option<String>,
    pub(super) cover: Option<std::path::PathBuf>,
    pub(super) artwork_cache: Entity<artwork::ThumbnailCache>,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) start_x: Pixels,
    pub(super) kind: SortList,
    pub(super) compact: bool,
    pub(super) trailing: String,
}
impl Render for DragPreview {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let desktop_track = self.kind == SortList::Tracks && !self.compact;
        let art_size = match self.kind {
            SortList::Tracks => {
                if self.compact {
                    56.
                } else {
                    48.
                }
            }
            SortList::Queue => 48.,
            SortList::Playlists => {
                if self.compact {
                    48.
                } else {
                    36.
                }
            }
        };
        let art = div()
            .size(px(art_size))
            .flex_shrink_0()
            .rounded(px(10.))
            .overflow_hidden()
            .bg(rgb(SIDEBAR))
            .when_some(self.cover.clone(), |el, path| {
                el.child(
                    img(path)
                        .image_cache(&self.artwork_cache)
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .rounded(px(10.)),
                )
            })
            .when(
                self.kind == SortList::Playlists && self.cover.is_none(),
                |el| {
                    let (from, to, icon) = match self.label.as_str() {
                        "All tracks" => (0x0ea5e9, 0x4338ca, IconName::ListMusic),
                        "Popular" => (0xffad00, 0xe00022, IconName::TrendingUp),
                        "Liked" => (0xdc00ff, 0x4731d9, IconName::Heart),
                        _ => (0x64748b, 0x1e293b, IconName::Music),
                    };
                    el.bg(linear_gradient(
                        135.,
                        linear_color_stop(rgb(from), 0.),
                        linear_color_stop(rgb(to), 1.),
                    ))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph(icon, 20.))
                },
            );
        div()
            .id("sortable-drag-preview")
            .test_support()
            .relative()
            .left(self.start_x - window.mouse_position().x)
            .w(px(self.width))
            .h(px(self.height))
            .px(px(if self.kind == SortList::Tracks {
                12.
            } else {
                4.
            }))
            .flex()
            .items_center()
            .gap_3()
            .rounded(px(12.))
            .bg(rgb(SECONDARY))
            .opacity(0.95)
            .shadow_md()
            .text_color(rgb(FG))
            .child(
                div()
                    .flex_basis(px(0.))
                    .flex_grow(if desktop_track { 1.4 } else { 1. })
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(art)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(self.label.clone()),
                            )
                            .when(!desktop_track, |el| {
                                el.when_some(self.artist.clone(), |el, artist| {
                                    el.child(muted(artist).text_xs().truncate())
                                })
                            }),
                    ),
            )
            .when(desktop_track, |el| {
                el.child(
                    muted(self.artist.clone().unwrap_or_default())
                        .flex_basis(px(0.))
                        .flex_grow(1.)
                        .min_w_0()
                        .truncate(),
                )
            })
            .when(self.kind != SortList::Queue, |el| {
                el.child(
                    muted(self.trailing.clone())
                        .w(px(if desktop_track {
                            72.
                        } else if self.kind == SortList::Tracks {
                            48.
                        } else {
                            24.
                        }))
                        .text_right()
                        .font_family(mono_font())
                        .flex_shrink_0(),
                )
            })
            .child(
                div()
                    .w(px(if self.kind == SortList::Tracks && self.compact {
                        48.
                    } else {
                        24.
                    }))
                    .flex_shrink_0(),
            )
    }
}
// HTML-style titles use GPUI's cursor-positioned, non-hoverable tooltip.
// Keep these separate from interactive popovers and immediate seek feedback.
pub(super) const HOVER_TITLE_DELAY: Duration = Duration::from_millis(1500);
pub(super) trait HoverTitle: InteractiveElement + Sized {
    fn hover_title(
        mut self,
        label: impl Into<SharedString>,
        epoch: &std::rc::Rc<std::cell::Cell<u64>>,
    ) -> Self {
        let label = label.into();
        let epoch = epoch.clone();
        let version = epoch.get();
        self.interactivity().tooltip_show_delay(HOVER_TITLE_DELAY);
        self.interactivity().tooltip(move |_, cx| {
            cx.new(|_| HoverTitleView {
                label: label.clone(),
                epoch: epoch.clone(),
                version,
            })
            .into()
        });
        self
    }
}
impl<E: InteractiveElement> HoverTitle for E {}
pub(super) struct HoverTitleView {
    pub(super) label: SharedString,
    pub(super) epoch: std::rc::Rc<std::cell::Cell<u64>>,
    pub(super) version: u64,
}
impl Render for HoverTitleView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if self.epoch.get() != self.version {
            return div().into_any_element();
        }
        div()
            // Native tooltip positioning starts at the cursor. Leave space for
            // the pointer, just like a browser title, without intercepting input.
            .pt(px(16.))
            .max_w((window.viewport_size().width - px(16.)).min(px(480.)))
            .child(
                div()
                    .id("hover-title")
                    .test_support()
                    .role(Role::Tooltip)
                    .aria_label(self.label.clone())
                    .px(px(6.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0x242424))
                    .text_color(rgb(FG))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .child(self.label.clone()),
            )
            .into_any_element()
    }
}
pub(super) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).rounded(px(12.)).secondary()
}
pub(super) fn glyph(name: IconName, size: f32) -> gpui_kit::component::Icon {
    let data: Option<&[u8]> = match name {
        IconName::Play => Some(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="white" d="M6 3a1 1 0 0 1 1.5-.86l14 8a1 1 0 0 1 0 1.72l-14 8A1 1 0 0 1 6 19z"/></svg>"#),
        IconName::Pause => Some(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect fill="white" x="5" y="3" width="5" height="18" rx="1"/><rect fill="white" x="14" y="3" width="5" height="18" rx="1"/></svg>"#),
        IconName::SkipBack => Some(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="white" stroke="white" stroke-width="2" stroke-linejoin="round" d="M20 4v16L8 12z"/><path stroke="white" stroke-width="2" stroke-linecap="round" d="M4 4v16"/></svg>"#),
        IconName::SkipForward => Some(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="white" stroke="white" stroke-width="2" stroke-linejoin="round" d="M4 4v16l12-8z"/><path stroke="white" stroke-width="2" stroke-linecap="round" d="M20 4v16"/></svg>"#),
        _ => None,
    };
    let icon = gpui_kit::component::Icon::new(name);
    if let Some(data) = data {
        icon.data(data).size(px(size)).text_color(rgb(0xffffff))
    } else {
        icon.size(px(size))
    }
}
// PopupMenu reserves a 12px icon slot with a 4px gap. Scaling its paint to
// 16px consumes 2px of that gap; add 4px to match develop's visible 6px gap.
pub(super) fn menu_icon(name: IconName) -> gpui_kit::component::Icon {
    glyph(name, 16.)
        .mr(px(4.))
        .transform(gpui::Transformation::scale(size(4. / 3., 4. / 3.)))
}
pub(super) fn filled_heart(size: f32) -> gpui_kit::component::Icon {
    gpui_kit::component::Icon::new(IconName::Heart).data(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="white" d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78L12 21.23l8.84-8.84a5.5 5.5 0 0 0 0-7.78z"/></svg>"#).size(px(size))
}
pub(super) fn icon(id: impl Into<ElementId>, name: IconName) -> Button {
    icon_size(id, name, 16.)
}
pub(super) fn icon_size(id: impl Into<ElementId>, name: IconName, size: f32) -> Button {
    let label = name
        .path()
        .to_string()
        .rsplit('/')
        .next()
        .unwrap_or("Control")
        .trim_end_matches(".svg")
        .replace('-', " ");
    Button::new(id)
        .child(glyph(name, size))
        .w(px(32.))
        .h(px(32.))
        .p_0()
        .accessibility_label(label)
        .ghost()
}
pub(super) fn thumbnail_progress(id: i64, ratio: f64) -> impl IntoElement {
    div()
        .id(("thumbnail-progress", id as usize))
        .test_support()
        .role(Role::ProgressIndicator)
        .aria_label(format!("Downloading {}%", (ratio * 100.).round()))
        .absolute()
        .inset_0()
        .child(
            canvas(
                move |bounds, _, _| bounds,
                move |_, bounds, window, _| {
                    let width = f32::from(bounds.size.width) - 1.;
                    let height = f32::from(bounds.size.height) - 1.;
                    let radius = 7.5_f32.min(width / 2.).min(height / 2.);
                    let mut points = vec![(radius, 0.)];
                    for (x, y, angle) in [
                        (width - radius, radius, -std::f32::consts::FRAC_PI_2),
                        (width - radius, height - radius, 0.),
                        (radius, height - radius, std::f32::consts::FRAC_PI_2),
                        (radius, radius, std::f32::consts::PI),
                    ] {
                        for step in 0..=8 {
                            let angle = angle + step as f32 / 8. * std::f32::consts::FRAC_PI_2;
                            points.push((x + radius * angle.cos(), y + radius * angle.sin()));
                        }
                    }
                    let length = points
                        .windows(2)
                        .map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1))
                        .sum::<f32>();
                    let mut remaining = length * ratio.clamp(0., 1.) as f32;
                    let point_at = |(x, y)| bounds.origin + point(px(x + 0.5), px(y + 0.5));
                    let mut path = PathBuilder::stroke(px(1.));
                    path.move_to(point_at(points[0]));
                    for p in points.windows(2) {
                        let segment = (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1);
                        let mix = if segment > 0. {
                            (remaining / segment).min(1.)
                        } else {
                            1.
                        };
                        path.line_to(point_at((
                            p[0].0 + (p[1].0 - p[0].0) * mix,
                            p[0].1 + (p[1].1 - p[0].1) * mix,
                        )));
                        remaining -= segment;
                        if remaining <= 0. {
                            break;
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(PRIMARY));
                    }
                },
            )
            .size_full(),
        )
}

pub(super) fn muted(value: impl Into<SharedString>) -> Stateful<Div> {
    let value = value.into();
    div()
        .id(value.clone())
        .role(Role::Label)
        .aria_label(value.clone())
        .text_sm()
        .line_height(px(20.))
        .text_color(rgb(MUTED))
        .child(value)
}
pub(super) fn heading(value: impl Into<SharedString>) -> Stateful<Div> {
    let value = value.into();
    div()
        .id(value.clone())
        .role(Role::Heading)
        .aria_label(value.clone())
        .text_base()
        .font_weight(FontWeight::SEMIBOLD)
        .child(value)
}
impl Render for PlayerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.has_active_drag() && self.sort_drag.take().is_some() {
            cx.notify();
        }
        if std::mem::take(&mut self.clear_track_search) {
            self.inputs
                .search
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        if std::mem::take(&mut self.reset_inputs) {
            for input in [
                &self.login_inputs(cx).phone,
                &self.login_inputs(cx).code,
                &self.login_inputs(cx).password,
                &self.inputs.search,
                &self.inputs.playlist_search,
                &self.inputs.name,
            ] {
                input.update(cx, |s, cx| s.set_value("", window, cx));
            }
        }
        if self.pending_back {
            self.pending_back = false;
            window.defer(cx, |window, cx| {
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            });
        }
        if self.fullscreen && self.panel.is_none() && window.focused(cx).is_none() {
            self.focus.focus(window, cx);
        }
        let modal_open = self
            .panel
            .as_ref()
            .is_some_and(|p| !matches!(p, Panel::Queue | Panel::PlaylistFilters));
        if modal_open != self.panel_was_open {
            self.panel_was_open = modal_open;
            if modal_open {
                self.panel_return_focus = window.focused(cx);
                if matches!(
                    self.panel,
                    Some(Panel::NewPlaylist | Panel::SaveQueue | Panel::RenamePlaylist(_))
                ) {
                    self.inputs
                        .name
                        .update(cx, |input, cx| input.focus(window, cx));
                } else if matches!(self.panel, Some(Panel::Search)) {
                    self.inputs
                        .search
                        .update(cx, |input, cx| input.focus(window, cx));
                } else {
                    self.modal_focus.focus(window, cx);
                }
            } else if let Some(focus) = self.panel_return_focus.take() {
                focus.focus(window, cx);
            }
        }
        let compact = window.viewport_size().width < px(768.);
        if self.volume_hover
            && (self.playback.read(cx).track_id.is_none() || (compact && !self.fullscreen))
        {
            self.reset_volume_hover();
        }
        self.sync_fullscreen_cursor(window, cx);
        let progress = if self.playback.read(cx).duration_seconds.is_finite()
            && self.playback.read(cx).duration_seconds > 0.
            && self.playback.read(cx).current_time_seconds.is_finite()
        {
            ((self.playback.read(cx).current_time_seconds
                / self.playback.read(cx).duration_seconds)
                .clamp(0., 1.)
                * 100.) as f32
        } else {
            0.
        };
        // Slider input updates its own state before its events reach the app.
        // Only push a fresh model value; a repaint must not undo a pointer move.
        let seek_value = self.seek_slider.read(cx).value().start();
        if self.scrubbing.is_none()
            && (self.synced_seek_percent != Some(progress) || !seek_value.is_finite())
        {
            self.synced_seek_percent = Some(progress);
            self.seek_slider
                .update(cx, |s, cx| s.set_value(progress, window, cx));
        }
        let volume = self.preferences.volume as f32;
        let volume_value = self.volume_slider.read(cx).value().start();
        if self.synced_volume_percent != Some(volume) || !volume_value.is_finite() {
            self.synced_volume_percent = Some(volume);
            self.volume_slider
                .update(cx, |s, cx| s.set_value(volume, window, cx));
        }
        let content = match self.phase {
            Phase::Loading => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(muted("Loading…"))
                .into_any_element(),
            Phase::Login => self.sign_in.clone().into_any_element(),
            Phase::Ready
                if self
                    .library
                    .read(cx)
                    .status
                    .as_ref()
                    .is_some_and(|s| s.blocking) =>
            {
                self.indexing_view(window, cx)
            }
            Phase::Ready if self.fullscreen => self.fullscreen_view(window, cx),
            Phase::Ready => div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(if compact {
                    if self.compact_playlist {
                        self.library_view(true, window, cx)
                    } else {
                        self.sidebar_panel(window, cx)
                    }
                } else {
                    self.desktop_library_view(window, cx)
                })
                .into_any_element(),
        };
        div()
            .id("soundgrammy")
            .key_context("SoundGrammy")
            .capture_action(cx.listener(|app, _: &Escape, window, cx| {
                if !app.cancel_sort_drag(window, cx) {
                    cx.propagate();
                }
            }))
            .capture_action(
                cx.listener(|app, _: &gpui_kit::base::actions::Cancel, window, cx| {
                    if !app.cancel_sort_drag(window, cx) {
                        cx.propagate();
                    }
                }),
            )
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(FG))
            .text_sm()
            .font_features(FontFeatures(Arc::new(vec![
                ("ss01".into(), 1),
                ("cv01".into(), 1),
            ])))
            .on_action(cx.listener(|app, _: &TogglePlayback, _, cx| {
                if app.playback.read(cx).track_id.is_some() {
                    app.command(Command::Toggle, cx);
                }
            }))
            .on_action(cx.listener(|app, _: &NextTrack, _, cx| app.command(Command::Next, cx)))
            .on_action(cx.listener(|app, _: &PreviousTrack, _, cx| {
                app.command(Command::Previous { restart: true }, cx)
            }))
            .on_action(cx.listener(|app, _: &SeekForward, _, cx| {
                app.seek(
                    model::seek(
                        app.playback.read(cx).current_time_seconds,
                        app.playback.read(cx).duration_seconds,
                        5.,
                    ),
                    cx,
                )
            }))
            .on_action(cx.listener(|app, _: &SeekBackward, _, cx| {
                app.seek(
                    model::seek(
                        app.playback.read(cx).current_time_seconds,
                        app.playback.read(cx).duration_seconds,
                        -5.,
                    ),
                    cx,
                )
            }))
            .on_action(cx.listener(|app, _: &Escape, window, cx| {
                app.dismiss_layer(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &About, window, cx| app.show_about(window, cx)))
            .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
            .on_action(cx.listener(|_, _: &Zoom, window, _| window.zoom_window()))
            .on_action(cx.listener(|app, _: &WindowFullscreen, window, cx| {
                if app.fullscreen {
                    app.toggle_fullscreen(window, cx)
                } else {
                    window.toggle_fullscreen();
                }
            }))
            .on_action(cx.listener(|app, _: &ToggleFullscreen, window, cx| {
                if app.playback.read(cx).track_id.is_some() {
                    app.toggle_fullscreen(window, cx);
                } else {
                    window.toggle_fullscreen();
                }
            }))
            .on_action(cx.listener(|app, _: &OpenSettings, window, cx| {
                app.open_panel(Panel::Settings, window, cx)
            }))
            .capture_key_down(cx.listener(|app, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && app.sort_drag.is_some() {
                    cx.stop_active_drag(window);
                    app.sort_drag = None;
                    cx.stop_propagation();
                    cx.notify();
                }
                if app.fullscreen {
                    app.show_fullscreen_controls(window, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _, window, cx| {
                    if app.fullscreen {
                        app.show_fullscreen_controls(window, cx);
                    }
                }),
            )
            .when(cfg!(target_os = "macos") && !window.is_fullscreen(), |el| {
                el.child(
                    TitleBar::new().h(px(32.)).bg(rgb(BG)).child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x666666))
                            .child(
                                crate::runtime::application_config()["productName"]
                                    .as_str()
                                    .unwrap_or("SoundGrammy")
                                    .to_owned(),
                            ),
                    ),
                )
            })
            .child(content)
            .when_some(
                self.error.clone().filter(|_| self.phase != Phase::Login),
                |el, error| {
                    el.child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(rgb(BG))
                            .text_color(rgb(ERROR))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().flex_1().child(error))
                            .child(button("retry-error", "Retry loading library").on_click(
                                cx.listener(|app, _, window, cx| {
                                    if app.host.is_none() {
                                        app.initialize(false, window, cx);
                                    } else if app.phase == Phase::Ready {
                                        app.hydrate(cx, false);
                                    } else {
                                        app.bootstrap(cx);
                                    }
                                }),
                            ))
                            .child(icon("dismiss-error", IconName::X).on_click(cx.listener(
                                |app, _, _, cx| {
                                    app.error = None;
                                    cx.notify();
                                },
                            ))),
                    )
                },
            )
            .when_some(self.notice.clone(), |el, notice| {
                el.child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(rgb(CARD))
                        .flex()
                        .items_center()
                        .child(div().flex_1().child(notice))
                        .child(icon("dismiss-notice", IconName::X).on_click(cx.listener(
                            |app, _, _, cx| {
                                app.notice = None;
                                cx.notify();
                            },
                        ))),
                )
            })
            .children(
                self.jobs
                    .values()
                    .map(|job| muted(job.clone()).px_4().py_1()),
            )
            .when(
                self.phase == Phase::Ready
                    && !self.fullscreen
                    && self.playback.read(cx).track_id.is_some(),
                |el| {
                    el.child(motion::Layer::cached(
                        "player-bar-view",
                        div()
                            .w_full()
                            .h(px(if compact { 80. } else { 96. }))
                            .flex_shrink_0()
                            .style()
                            .clone(),
                        &self.playback,
                        self.player_bar_inputs(cx),
                        window,
                        cx,
                        move |app, window, cx| app.player_bar(compact, window, cx),
                    ))
                },
            )
            .child(motion::Layer::bind(
                "compact-player-layer",
                window,
                cx,
                Self::compact_player_layer,
            ))
            .child(motion::Layer::bind(
                "modal-layer",
                window,
                cx,
                Self::modal_layer,
            ))
    }
}

#[derive(Clone, Copy)]
pub(super) struct SidebarResize;
impl Render for SidebarResize {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}
