use super::menus::MenuGroups;
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::base::slider::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    slider::Slider,
    Disableable, Sizable, TitleBar,
};
pub(super) const BG: u32 = 0x141414;
pub(super) const CARD: u32 = 0x202020;
pub(super) const SIDEBAR: u32 = 0x191919;
pub(super) const FG: u32 = 0xf2f2f2;
pub(super) const MUTED: u32 = 0xa5a5a5;
pub(super) const PRIMARY: u32 = 0x2aabee;
pub(super) const BORDER: u32 = 0x383838;
#[derive(Clone)]
struct TrackDrag {
    index: usize,
    playlist: PlaylistId,
}
#[derive(Clone)]
struct QueueDrag {
    index: usize,
    revision: u64,
}
#[derive(Clone)]
struct PlaylistDrag(PlaylistId);
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SortList {
    Tracks,
    Queue,
    Playlists,
}
#[derive(Clone, Copy)]
pub(super) struct SortDrag {
    pub list: SortList,
    pub from: usize,
    pub over: usize,
}
fn drag_shift(drag: Option<SortDrag>, list: SortList, index: usize, stride: f32) -> f32 {
    match drag.filter(|d| d.list == list) {
        Some(d) if d.from < d.over && index > d.from && index <= d.over => -stride,
        Some(d) if d.from > d.over && index >= d.over && index < d.from => stride,
        _ => 0.,
    }
}
struct DragPreview {
    label: String,
    artist: Option<String>,
    cover: Option<std::path::PathBuf>,
    artwork_cache: Entity<artwork::ThumbnailCache>,
    width: f32,
    height: f32,
    start_x: Pixels,
    kind: SortList,
    compact: bool,
    trailing: String,
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
            .bg(rgb(0x272727))
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
struct HoverTitleView {
    label: SharedString,
    epoch: std::rc::Rc<std::cell::Cell<u64>>,
    version: u64,
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
fn menu_icon(name: IconName) -> gpui_kit::component::Icon {
    glyph(name, 16.)
        .mr(px(4.))
        .transform(gpui::Transformation::scale(size(4. / 3., 4. / 3.)))
}
fn filled_heart(size: f32) -> gpui_kit::component::Icon {
    gpui_kit::component::Icon::new(IconName::Heart).data(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="white" d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78L12 21.23l8.84-8.84a5.5 5.5 0 0 0 0-7.78z"/></svg>"#).size(px(size))
}
pub(super) fn icon(id: impl Into<ElementId>, name: IconName) -> Button {
    icon_size(id, name, 16.)
}
fn icon_size(id: impl Into<ElementId>, name: IconName, size: f32) -> Button {
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
fn thumbnail_progress(id: i64, ratio: f64) -> impl IntoElement {
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
        if self.snapshot.initial_loading && !self.snapshot.seeking {
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
        if std::mem::take(&mut self.clear_track_search) {
            self.inputs
                .search
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        if std::mem::take(&mut self.reset_inputs) {
            for input in [
                &self.inputs.phone,
                &self.inputs.code,
                &self.inputs.password,
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
        if (self.phase == Phase::Login || self.snapshot.track_id.is_none()) && self.fullscreen {
            self.toggle_fullscreen(window, cx);
        }
        let compact = window.viewport_size().width < px(768.);
        if self.volume_hover && (self.snapshot.track_id.is_none() || (compact && !self.fullscreen))
        {
            self.reset_volume_hover();
        }
        self.sync_fullscreen_cursor(window, cx);
        let player = self.snapshot.player.clone();
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
        if self.pending_fullscreen && !self.fullscreen_artwork_pending() {
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
        let progress = if self.snapshot.duration_seconds.is_finite()
            && self.snapshot.duration_seconds > 0.
            && self.snapshot.current_time_seconds.is_finite()
        {
            ((self.snapshot.current_time_seconds / self.snapshot.duration_seconds).clamp(0., 1.)
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
            Phase::Login => self.login_view(window, cx),
            Phase::Ready if self.library.status.as_ref().is_some_and(|s| s.blocking) => {
                self.indexing_view(window, cx)
            }
            Phase::Ready if self.fullscreen => self.fullscreen_view(window, cx),
            Phase::Ready => {
                let cover_ids = self
                    .library
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
                let sidebar = |window: &mut Window, cx: &mut Context<Self>| {
                    motion::Layer::cached(
                        "sidebar-view",
                        div().size_full().style().clone(),
                        window,
                        cx,
                        Self::sidebar_view,
                    )
                };
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(if compact {
                        if self.compact_playlist {
                            self.library_view(compact, window, cx)
                        } else {
                            sidebar(window, cx).into_any_element()
                        }
                    } else {
                        div()
                            .size_full()
                            .flex()
                            .child(
                                div()
                                    .w(px(self.preferences.sidebar_width))
                                    .min_w(px(220.))
                                    .max_w(px(480.))
                                    .h_full()
                                    .child(sidebar(window, cx)),
                            )
                            .child(
                                div()
                                    .w(px(1.))
                                    .h_full()
                                    .bg(rgb(BORDER))
                                    .cursor_col_resize()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|app, _, _, cx| {
                                            app.drag_track = Some(usize::MAX);
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(self.library_view(compact, window, cx)),
                            )
                            .into_any_element()
                    })
                    .into_any_element()
            }
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
                if app.snapshot.track_id.is_some() {
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
                        app.snapshot.current_time_seconds,
                        app.snapshot.duration_seconds,
                        5.,
                    ),
                    cx,
                )
            }))
            .on_action(cx.listener(|app, _: &SeekBackward, _, cx| {
                app.seek(
                    model::seek(
                        app.snapshot.current_time_seconds,
                        app.snapshot.duration_seconds,
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
                if app.snapshot.track_id.is_some() {
                    app.toggle_fullscreen(window, cx);
                } else {
                    window.toggle_fullscreen();
                }
            }))
            .on_action(cx.listener(|app, _: &OpenSettings, window, cx| {
                app.open_panel(Panel::Settings, window, cx)
            }))
            .on_mouse_move(cx.listener(|app, event: &MouseMoveEvent, _, cx| {
                if app.drag_track == Some(usize::MAX) {
                    app.preferences.sidebar_width = f32::from(event.position.x).clamp(220., 480.);
                    cx.notify();
                }
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
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|app, _, _, _| {
                    if app.drag_track == Some(usize::MAX) {
                        app.drag_track = None;
                        app.save_preferences();
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
                            .text_color(rgb(0xff656a))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().flex_1().child(error))
                            .child(button("retry-error", "Retry loading library").on_click(
                                cx.listener(|app, _, _, cx| {
                                    if app.phase == Phase::Ready {
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
                self.phase == Phase::Ready && !self.fullscreen && self.snapshot.track_id.is_some(),
                |el| {
                    el.child(motion::Layer::cached(
                        "player-bar-view",
                        div()
                            .w_full()
                            .h(px(if compact { 80. } else { 96. }))
                            .flex_shrink_0()
                            .style()
                            .clone(),
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
impl PlayerApp {
    fn compact_player_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let compact_open = matches!(self.panel, Some(Panel::CompactPlayer))
            || self
                .panel_stack
                .iter()
                .any(|p| matches!(p, Panel::CompactPlayer));
        let compact_motion =
            motion::presence("player-sheet-motion", compact_open, true, window, cx);
        if (compact_open || compact_motion.progress > 0.) && self.snapshot.track_id.is_some() {
            self.compact_player_view(compact_motion, window, cx)
        } else {
            Empty.into_any_element()
        }
    }

    fn modal_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Compact search is a non-modal page above the persistent player, just
        // like develop: no backdrop, dialog padding or exit overlay.
        if matches!(self.panel, Some(Panel::Search)) {
            self.retained_modal = None;
            return self.search_overlay(window, cx);
        }
        let active_modal = self.panel.as_ref().filter(|p| {
            !matches!(
                p,
                Panel::Queue | Panel::PlaylistFilters | Panel::CompactPlayer
            )
        });
        let docked = matches!(
            active_modal.or(self.retained_modal.as_ref()),
            Some(Panel::Navigation)
        );
        let modal = motion::presence("modal-motion", active_modal.is_some(), docked, window, cx);
        if let Some(panel) = active_modal {
            self.retained_modal = Some(panel.clone());
        } else if !modal.should_render() {
            self.retained_modal = None;
        }
        self.modal_progress = modal.progress;
        #[cfg(target_os = "macos")]
        let blur = self
            .retained_modal
            .as_ref()
            .filter(|p| !matches!(p, Panel::Volume))
            .and(self.blur_surface.clone());
        #[cfg(target_os = "macos")]
        if let Some(surface) = &blur {
            surface.update(gpui_apple::effects::Effect::BackdropBlur {
                radius: 16. * window.scale_factor(),
                opacity: modal.progress,
            });
        }
        div()
            .absolute()
            .inset_0()
            .child({
                #[cfg(target_os = "macos")]
                {
                    blur.map_or_else(
                        || Empty.into_any_element(),
                        |surface| {
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    window.paint_surface(bounds, surface.buffer())
                                },
                            )
                            .absolute()
                            .top(px(if window.is_fullscreen() { 0. } else { 32. }))
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .into_any_element()
                        },
                    )
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Empty.into_any_element()
                }
            })
            .when_some(self.retained_modal.clone(), |el, panel| {
                el.child(self.panel_view(panel, window, cx))
            })
            .into_any_element()
    }

    fn cancel_sort_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.sort_drag.is_none() {
            return false;
        }
        cx.stop_active_drag(window);
        self.sort_drag = None;
        cx.stop_propagation();
        cx.notify();
        true
    }
    pub(super) fn dismiss_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_fullscreen {
            self.pending_fullscreen = false;
            cx.notify();
        } else if self.volume_exact_open {
            self.volume_exact_open = false;
            self.focus.focus(window, cx);
        } else if self.panel.is_some() {
            self.close_panel(window, cx);
        } else if self.fullscreen {
            self.toggle_fullscreen(window, cx);
        } else if self.selection_mode {
            self.selection_mode = false;
            self.library.clear_selection();
        } else if self.compact_playlist {
            self.compact_playlist = false;
        } else {
            #[cfg(target_os = "android")]
            let _ = crate::android_keyring::background_activity();
        }
    }
    fn indexing_view(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (done, total) = self.sync_progress.unwrap_or_else(|| {
            self.library
                .status
                .as_ref()
                .map(|s| (s.processed, s.total))
                .unwrap_or_default()
        });
        let fraction = if total > 0 {
            (done as f32 / total as f32).clamp(0., 1.)
        } else {
            0.3
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div().p_4().child(
                    icon("navigation", IconName::Menu)
                        .accessibility_label("Menu")
                        .hover_title("Menu", &self.hover_title_epoch)
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.open_panel(Panel::Navigation, window, cx)
                        })),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .child(
                        div()
                            .w(px(384.))
                            .max_w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_4()
                            .text_center()
                            .child(motion::Layer::bind(
                                "indexing-spinner-layer",
                                window,
                                cx,
                                |_, _, _| {
                                    gpui_kit::component::spinner::Spinner::new()
                                        .icon(IconName::LoaderCircle)
                                        .color(rgb(PRIMARY).into())
                                        .with_size(px(32.))
                                        .into_any_element()
                                },
                            ))
                            .child(heading("Indexing your library").text_lg())
                            .child(muted(if total > 0 {
                                format!("{done} of {total} tracks indexed")
                            } else {
                                "Preparing your library…".into()
                            }))
                            .child(
                                div()
                                    .id("indexing-progress")
                                    .test_support()
                                    .role(Role::ProgressIndicator)
                                    .aria_label("Library indexing progress")
                                    .h(px(8.))
                                    .w_full()
                                    .rounded_full()
                                    .bg(rgb(0x272727))
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(fraction))
                                            .rounded_full()
                                            .bg(rgb(PRIMARY)),
                                    ),
                            )
                            .child(muted("Your tracks will appear when indexing finishes."))
                            .when(self.error.is_some(), |el| {
                                el.child(
                                    button("index-retry", "Retry indexing")
                                        .disabled(self.syncing)
                                        .on_click(cx.listener(|app, _, _, cx| app.sync(false, cx))),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
    fn playlist_cover(&self, id: &PlaylistId, compact: bool) -> AnyElement {
        let (from, to, name) = match id {
            PlaylistId::All => (0x0ea5e9, 0x4338ca, IconName::ListMusic),
            PlaylistId::Builtin(id) if id == "liked" => (0xdc00ff, 0x4731d9, IconName::Heart),
            PlaylistId::Builtin(id) if id == "popular" => {
                (0xffad00, 0xe00022, IconName::TrendingUp)
            }
            PlaylistId::Builtin(_) => (0x10b981, 0x0e7490, IconName::Clock),
            _ => (0x64748b, 0x1e293b, IconName::Music),
        };
        let tracks = if let PlaylistId::Custom(id) = id {
            self.library
                .playlists
                .as_ref()
                .and_then(|p| p.custom.iter().find(|p| p.id == *id))
                .map(|p| p.track_ids.iter().take(4).copied().collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            vec![]
        };
        let outer = div()
            .size(px(if compact { 48. } else { 36. }))
            .flex_shrink_0()
            .relative()
            .rounded(px(8.))
            .overflow_hidden()
            .bg(linear_gradient(
                135.,
                linear_color_stop(rgb(from), 0.),
                linear_color_stop(rgb(to), 1.),
            ));
        if tracks.is_empty() {
            outer
                .flex()
                .items_center()
                .justify_center()
                .child(if name == IconName::Heart {
                    filled_heart(20.)
                } else {
                    glyph(name, 20.)
                })
                .into_any_element()
        } else {
            outer
                .children(tracks.iter().enumerate().map(|(index, id)| {
                    let count = tracks.len();
                    let left = if count == 3 {
                        if index == 0 {
                            0.
                        } else {
                            0.5
                        }
                    } else if count == 1 {
                        0.
                    } else {
                        (index % 2) as f32 * 0.5
                    };
                    let top = if count == 3 {
                        if index < 2 {
                            0.
                        } else {
                            0.5
                        }
                    } else if count <= 2 {
                        0.
                    } else {
                        (index / 2) as f32 * 0.5
                    };
                    div()
                        .absolute()
                        .left(relative(left))
                        .top(relative(top))
                        .w(relative(if count == 1 { 1. } else { 0.5 }))
                        .h(relative(if count <= 2 || (count == 3 && index == 0) {
                            1.
                        } else {
                            0.5
                        }))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when_some(self.thumbs.get(id).cloned().flatten(), |el, path| {
                            el.child(
                                img(path)
                                    .image_cache(&self.artwork_cache)
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .when(left == 0. && top == 0., |el| el.rounded_tl(px(8.)))
                                    .when(left > 0. && top == 0. || count == 1, |el| {
                                        el.rounded_tr(px(8.))
                                    })
                                    .when(
                                        left == 0.
                                            && (top > 0. || count <= 2 || count == 3 && index == 0),
                                        |el| el.rounded_bl(px(8.)),
                                    )
                                    .when(
                                        left > 0. && (top > 0. || count <= 2) || count == 1,
                                        |el| el.rounded_br(px(8.)),
                                    ),
                            )
                        })
                        .when(self.thumbs.get(id).cloned().flatten().is_none(), |el| {
                            el.child(gpui_kit::component::Icon::new(IconName::Music).size(px(12.)))
                        })
                }))
                .into_any_element()
        }
    }
    fn sidebar_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let compact = window.viewport_size().width < px(768.);
        let control = if compact { 48. } else { 32. };
        let sidebar_width = self.preferences.sidebar_width;
        let items = self
            .library
            .items(&self.preferences)
            .into_iter()
            .filter(|p| {
                self.statistics_enabled
                    || !matches!(&p.id, PlaylistId::Builtin(id) if id=="popular" || id=="recent")
            })
            .filter(|p| {
                p.name
                    .to_lowercase()
                    .contains(&self.playlist_search.to_lowercase())
            })
            .collect::<Vec<_>>();
        let view = cx.entity();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(SIDEBAR))
            .pt_4()
            .gap_4()
            .child(
                div()
                    .px_4()
                    .flex()
                    .items_center()
                    .h(px(control))
                    .gap(px(if compact { 16. } else { 8. }))
                    .child(
                        icon_size(
                            "navigation",
                            IconName::Menu,
                            if compact { 24. } else { 16. },
                        )
                        .w(px(control))
                        .h(px(control))
                        .mr(px(if compact { 0. } else { 8. }))
                        .accessibility_label("Menu")
                        .hover_title("Menu", &self.hover_title_epoch)
                        .flex_shrink_0()
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.open_panel(Panel::Navigation, window, cx)
                        })),
                    )
                    .child(
                        heading("Library")
                            .test_support()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex_1()
                            .min_w_0()
                            .truncate(),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(if compact { 8. } else { 2. }))
                            .child(self.filters_button(compact, window, cx))
                            .child(
                                icon_size(
                                    "new-playlist",
                                    IconName::Plus,
                                    if compact { 24. } else { 16. },
                                )
                                .w(px(control))
                                .h(px(control))
                                .on_click(cx.listener(
                                    |app, _, window, cx| {
                                        app.open_panel(Panel::NewPlaylist, window, cx)
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .id("playlist-sidebar-scroll")
                    .px_2()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(items.into_iter().enumerate().map(|(index, item)| {
                        let id = item.id.clone();
                        let selected = self.library.selected == id;
                        let label = item.name.clone();
                        let preview_count = item.count.to_string();
                        let preview_cover = if let PlaylistId::Custom(id) = item.id {
                            self.library
                                .playlists
                                .as_ref()
                                .and_then(|p| p.custom.iter().find(|p| p.id == id))
                                .and_then(|p| p.track_ids.first())
                                .and_then(|id| self.thumbs.get(id))
                                .cloned()
                                .flatten()
                        } else {
                            None
                        };
                        let context_view = view.clone();
                        let context_id = id.clone();
                        let drag = id.clone();
                        let drop_id = id.clone();
                        let drag_view = view.clone();
                        let sortable = self.preferences.playlist_sort == "custom"
                            && self.playlist_search.is_empty();
                        let row = gpui_kit::base::Button::new(SharedString::from(format!(
                            "playlist-{}",
                            id.key()
                        )))
                        .group("playlist-row")
                        .relative()
                        .top(px(drag_shift(
                            self.sort_drag,
                            SortList::Playlists,
                            index,
                            if compact { 74. } else { 54. },
                        )))
                        .when(
                            self.sort_drag
                                .is_some_and(|d| d.list == SortList::Playlists && d.from == index),
                            |el| el.opacity(0.),
                        )
                        .accessibility_label(format!("Select {} playlist", item.name))
                        .hover_title(item.name.clone(), &self.hover_title_epoch)
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_2()
                        .py_2()
                        .min_h(px(if window.viewport_size().width < px(768.) {
                            72.
                        } else {
                            52.
                        }))
                        .mb(px(2.))
                        .rounded(px(12.))
                        .cursor_pointer()
                        .when(selected, |el| el.bg(rgb(0x203039)))
                        .hover(|el| el.bg(rgb(CARD)))
                        .child(
                            self.playlist_cover(&item.id, window.viewport_size().width < px(768.)),
                        )
                        .child(
                            div().flex_1().min_w_0().child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(item.name),
                            ),
                        )
                        .child(
                            muted(item.count.to_string())
                                .font_family(mono_font())
                                .text_xs()
                                .font_weight(FontWeight::NORMAL)
                                .flex_shrink_0()
                                .mr(px(-8.)),
                        )
                        .child({
                            let menu_view = view.clone();
                            let menu_id = item.id.clone();
                            div()
                                .id(SharedString::from(format!(
                                    "playlist-option-holder-{}",
                                    menu_id.key()
                                )))
                                .when(window.viewport_size().width >= px(768.), |el| {
                                    el.opacity(0.)
                                })
                                .group_hover("playlist-row", |el| el.opacity(1.))
                                .flex_shrink_0()
                                .w(px(if window.viewport_size().width < px(768.) {
                                    36.
                                } else {
                                    24.
                                }))
                                .child(div().when(item.id != PlaylistId::All, |el| {
                                    el.child(
                                        icon(
                                            SharedString::from(format!(
                                                "playlist-options-{}",
                                                menu_id.key()
                                            )),
                                            IconName::Ellipsis,
                                        )
                                        .w(px(24.))
                                        .h(px(24.))
                                        .dropdown_menu(
                                            move |menu, _, _| {
                                                Self::playlist_menu(
                                                    menu,
                                                    menu_view.clone(),
                                                    menu_id.clone(),
                                                )
                                            },
                                        ),
                                    )
                                }))
                        })
                        .on_click(
                            cx.listener(move |app, _, _, cx| app.select_playlist(id.clone(), cx)),
                        )
                        .when(sortable, |el| {
                            el.on_drag(PlaylistDrag(drag), move |_, _, window, cx| {
                                let artwork_cache = drag_view.update(cx, |app, cx| {
                                    app.sort_drag = Some(SortDrag {
                                        list: SortList::Playlists,
                                        from: index,
                                        over: index,
                                    });
                                    cx.notify();
                                    app.artwork_cache.clone()
                                });
                                let start_x = window.mouse_position().x;
                                cx.new(|_| DragPreview {
                                    artwork_cache,
                                    label: label.clone(),
                                    artist: None,
                                    cover: preview_cover.clone(),
                                    width: if compact {
                                        f32::from(window.viewport_size().width) - 16.
                                    } else {
                                        sidebar_width - 16.
                                    },
                                    height: if compact { 72. } else { 52. },
                                    start_x,
                                    kind: SortList::Playlists,
                                    compact,
                                    trailing: preview_count.clone(),
                                })
                            })
                        })
                        .context_menu(move |menu, _, _| {
                            Self::playlist_menu(menu, context_view.clone(), context_id.clone())
                        })
                        .into_any_element();
                        div()
                            .h(px(if compact { 74. } else { 54. }))
                            .w_full()
                            .when(sortable, |el| {
                                el.on_drag_move(cx.listener(
                                    move |app, event: &DragMoveEvent<PlaylistDrag>, _, cx| {
                                        if !event.bounds.contains(&event.event.position) {
                                            return;
                                        }
                                        let drag = event.drag(cx);
                                        let order = app.library.items(&app.preferences);
                                        if let Some(from) =
                                            order.iter().position(|p| p.id == drag.0)
                                        {
                                            if app.sort_drag.is_none_or(|d| d.over != index) {
                                                app.sort_drag = Some(SortDrag {
                                                    list: SortList::Playlists,
                                                    from,
                                                    over: index,
                                                });
                                                cx.notify();
                                            }
                                        }
                                    },
                                ))
                                .on_drop(cx.listener(
                                    move |app, drag: &PlaylistDrag, _, cx| {
                                        app.sort_drag = None;
                                        let mut order = app
                                            .library
                                            .items(&app.preferences)
                                            .into_iter()
                                            .map(|p| p.id.key())
                                            .collect::<Vec<_>>();
                                        if let (Some(from), Some(to)) = (
                                            order.iter().position(|p| p == &drag.0.key()),
                                            order.iter().position(|p| p == &drop_id.key()),
                                        ) {
                                            let value = order.remove(from);
                                            order.insert(to, value);
                                            app.preferences.custom_order = order;
                                            app.preferences.playlist_sort = "custom".into();
                                            app.save_preferences();
                                            cx.notify();
                                        }
                                    },
                                ))
                            })
                            .child(row)
                    })),
            )
            .into_any_element()
    }
    fn playlist_menu(
        menu: PopupMenu,
        context_view: Entity<Self>,
        context_id: PlaylistId,
    ) -> PopupMenu {
        let mut menu = menu.min_w(px(176.)).max_w(px(256.));
        if let PlaylistId::Custom(id) = context_id {
            let v = context_view.clone();
            menu = menu.item(
                PopupMenuItem::new("Edit playlist")
                    .icon(menu_icon(IconName::Pencil))
                    .on_click(move |_, window, cx| {
                        v.update(cx, |app, cx| {
                            app.open_panel(Panel::RenamePlaylist(id), window, cx);
                        })
                    }),
            );
            let v = context_view.clone();
            menu = menu.item(
                PopupMenuItem::new("Delete playlist")
                    .icon(menu_icon(IconName::Trash))
                    .on_click(move |_, _, cx| {
                        v.update(cx, |app, cx| {
                            app.show_panel(Panel::Confirm(Confirmation::DeletePlaylist(id)));
                            cx.notify();
                        })
                    }),
            );
        }
        if let PlaylistId::Builtin(ref id) = context_id {
            let id = id.clone();
            let v = context_view.clone();
            menu = menu.item(
                PopupMenuItem::new("Hide playlist")
                    .icon(menu_icon(IconName::EyeOff))
                    .on_click(move |_, _, cx| {
                        v.update(cx, |app, cx| {
                            app.preferences.hidden.insert(id.clone());
                            if app.library.selected == PlaylistId::Builtin(id.clone()) {
                                app.library.select(PlaylistId::All);
                                app.preferences.selected = "all".into();
                                app.clear_track_search = true;
                            }
                            app.save_preferences();
                            cx.notify();
                        })
                    }),
            );
        }
        if (matches!(context_id, PlaylistId::Custom(_))
            || context_id == PlaylistId::Builtin("liked".into()))
        {
            let v = context_view.clone();
            let id = context_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Export playlist")
                    .icon(menu_icon(IconName::FileDown))
                    .on_click(move |_, _, cx| {
                        v.update(cx, |app, cx| {
                            app.export_recipe(id.clone(), cx);
                        })
                    }),
            );
        }
        menu
    }
    fn empty_playlist_view(&self) -> AnyElement {
        let (title, description) = if self.library.tracks.is_empty() {
            (
                "No tracks yet",
                "Pin music to your Telegram profile and it will tune in here automatically.",
            )
        } else {
            match &self.library.selected {
                PlaylistId::Builtin(id) if id == "liked" => (
                    "No liked tracks yet",
                    "Tap the heart on any track to save it here.",
                ),
                PlaylistId::Builtin(id) if id == "popular" || id == "recent" => (
                    "No listening history yet",
                    "Play some tracks and they will show up here.",
                ),
                PlaylistId::Custom(_) => (
                    "This playlist is empty",
                    "Add tracks from your library using the list button.",
                ),
                _ => (
                    "No tracks yet",
                    "Pin music to your Telegram profile and it will tune in here automatically.",
                ),
            }
        };
        div()
            .id("playlist-empty-state")
            .test_support()
            .w_full()
            .px(px(24.))
            .py(px(80.))
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .text_center()
            .child(
                div()
                    .size(px(56.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(CARD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph(IconName::Music, 24.).text_color(rgb(MUTED))),
            )
            .child(
                heading(title)
                    .id("playlist-empty-title")
                    .test_support()
                    .font_weight(FontWeight::MEDIUM),
            )
            .child(muted(description).max_w(px(320.)))
            .into_any_element()
    }
    fn library_view(
        &mut self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if matches!(self.panel, Some(Panel::Search)) {
            return div().size_full().into_any_element();
        }
        let list = self.library.track_list();
        let entries = list.entries;
        let count = entries.len();
        let total = list.membership.len();
        let visible = entries.clone();
        let active_row = self.library.playing_source_index(&self.snapshot);
        let has_selection = self.selection_mode
            && !self.library.selection.is_empty()
            && entries
                .iter()
                .any(|entry| self.library.selection.contains(&entry.source_index));
        // Match develop's compact toolbar: shuffle appears at 480px,
        // cache/download at 640px; narrower windows keep them in the menu.
        let width = window.viewport_size().width;
        let show_shuffle = !compact || width >= px(480.);
        let show_storage_actions = !compact || width >= px(640.);
        let can_download = !matches!(&self.library.selected, PlaylistId::Builtin(id) if id == "popular" || id == "recent");
        let cache_disabled = count == 0
            || entries
                .iter()
                .all(|entry| self.library.cached.contains(&entry.track.id));
        let button_size = if compact { 48. } else { 32. };
        let symbol_size = if compact { 24. } else { 16. };
        let actions = div()
            .flex()
            .items_center()
            .gap(px(if compact { 4. } else { 10. }))
            .when(!compact, |el| el.w_full())
            .when(compact, |el| el.flex_shrink_0())
            .when(!self.selection_mode && total > 0, |el| {
                el.child(
                    Button::new("play-playlist")
                        .primary()
                        .child(glyph(IconName::Play, symbol_size))
                        .size(px(button_size))
                        .p_0()
                        .accessibility_label("Play playlist")
                        .hover_title("Play", &self.hover_title_epoch)
                        .disabled(count == 0)
                        .on_click(cx.listener(|app, _, _, cx| app.play_entries(0, None, cx))),
                )
                .when(show_shuffle, |el| {
                    el.child(
                        icon_size("shuffle-playlist", IconName::Shuffle, symbol_size)
                            .size(px(button_size))
                            .hover_title("Shuffle", &self.hover_title_epoch)
                            .bg(rgb(CARD))
                            .rounded(px(12.))
                            .disabled(count == 0)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.play_entries(0, Some(audio::session::Shuffle::On), cx)
                            })),
                    )
                })
                .when(show_storage_actions, |el| {
                    el.child(
                        icon_size("cache-playlist", IconName::HardDriveDownload, symbol_size)
                            .size(px(button_size))
                            .hover_title("Cache playlist", &self.hover_title_epoch)
                            .bg(rgb(CARD))
                            .rounded(px(12.))
                            .disabled(cache_disabled)
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.track_action(TrackAction::Cache, app.library.entries(), cx)
                            })),
                    )
                    .when(can_download, |el| {
                        el.child(
                            icon_size("download-playlist", IconName::Download, symbol_size)
                                .size(px(button_size))
                                .hover_title("Download playlist", &self.hover_title_epoch)
                                .bg(rgb(CARD))
                                .rounded(px(12.))
                                .disabled(count == 0)
                                .on_click(cx.listener(|app, _, _, cx| app.download_playlist(cx))),
                        )
                    })
                })
                .when(!show_storage_actions, |el| {
                    let view = cx.entity();
                    el.child(
                        icon_size("more-playlist-actions", IconName::Ellipsis, 20.)
                            .size(px(button_size))
                            .accessibility_label("More playlist actions")
                            .hover_title("More playlist actions", &self.hover_title_epoch)
                            .dropdown_menu(move |mut menu, _, _| {
                                menu = menu.min_w(px(192.));
                                if !show_shuffle {
                                    let view = view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new("Shuffle")
                                            .icon(menu_icon(IconName::Shuffle))
                                            .disabled(count == 0)
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |app, cx| {
                                                    app.play_entries(
                                                        0,
                                                        Some(audio::session::Shuffle::On),
                                                        cx,
                                                    )
                                                });
                                            }),
                                    );
                                }
                                let cache_view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new("Cache playlist")
                                        .icon(menu_icon(IconName::HardDriveDownload))
                                        .disabled(cache_disabled)
                                        .on_click(move |_, _, cx| {
                                            cache_view.update(cx, |app, cx| {
                                                app.track_action(
                                                    TrackAction::Cache,
                                                    app.library.entries(),
                                                    cx,
                                                )
                                            });
                                        }),
                                );
                                if can_download {
                                    let view = view.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new("Download playlist")
                                            .icon(menu_icon(IconName::Download))
                                            .disabled(count == 0)
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |app, cx| {
                                                    app.download_playlist(cx)
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                })
            })
            .when(self.selection_mode && total > 0, |el| {
                el.when(has_selection, |el| {
                    el.child(self.bulk_actions_button(compact, cx))
                })
                .child(
                    icon_size("exit-selection", IconName::Undo2, symbol_size)
                        .size(px(button_size))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .rounded(px(12.))
                        .accessibility_label("Exit selection")
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.selection_mode = false;
                            app.library.clear_selection();
                            cx.notify();
                        })),
                )
            })
            .when(!compact, |el| {
                el.child(
                    div().flex_1().min_w_0().child(
                        Input::new(&self.inputs.search)
                            .cleanable(true)
                            .prefix(gpui_kit::component::Icon::new(IconName::Search))
                            .id("track-search")
                            .aria_label("Search tracks"),
                    ),
                )
            })
            .when(compact, |el| {
                el.child(
                    icon_size("open-track-search", IconName::Search, 24.)
                        .size(px(48.))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .rounded(px(16.))
                        .accessibility_label("Search tracks")
                        .hover_title(
                            if self.library.search.is_empty() {
                                "Search tracks".into()
                            } else {
                                format!("Search: {}", self.library.search)
                            },
                            &self.hover_title_epoch,
                        )
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.open_panel(Panel::Search, window, cx)
                        })),
                )
            });
        let header = if compact {
            div()
                .h(px(64.))
                .flex_shrink_0()
                .px_2()
                .mb_3()
                .border_b_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SIDEBAR))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    icon_size("back", IconName::ArrowLeft, 24.)
                        .w(px(48.))
                        .h(px(48.))
                        .accessibility_label("Back to playlists")
                        .hover_title("Back to playlists", &self.hover_title_epoch)
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.compact_playlist = false;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .id("playlist-heading")
                        .test_support()
                        .overflow_hidden()
                        .child(
                            heading(self.library.name())
                                .truncate()
                                .hover_title(self.library.name(), &self.hover_title_epoch),
                        )
                        .child(
                            muted(format!("{total} tracks"))
                                .id("playlist-track-count")
                                .test_support()
                                .whitespace_nowrap()
                                .text_xs()
                                .line_height(px(16.)),
                        ),
                )
                .child(actions)
                .into_any_element()
        } else {
            div().px_4().mb_4().child(actions).into_any_element()
        };
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .pt(px(if compact { 0. } else { 16. }))
            .when(compact || total > 0, |el| el.child(header));
        if total == 0 {
            return content.child(self.empty_playlist_view()).into_any_element();
        }
        content
            .child(
                gpui_kit::base::Table::new("tracks-table")
                    .accessibility_label(format!("{} tracks", self.library.name()))
                    .row_count(count + 1)
                    .column_count(if compact { 3 } else { 4 } + usize::from(self.selection_mode))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        gpui_kit::base::TableHeader::new("track-header")
                            .flex_shrink_0()
                            .child(
                                gpui_kit::base::TableRow::new("track-header-row", 0)
                                    .mx(px(if compact { 8. } else { 16. }))
                                    .px(px(if compact { 12. } else { 14. }))
                                    .mb_2()
                                    .flex()
                                    .items_center()
                                    .gap(px(if compact { 8. } else { 12. }))
                                    .h(px(if compact { 48. } else { 36. }))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .bg(rgb(0x191919))
                                    .rounded(px(12.))
                                    .when(self.selection_mode, |el| {
                                        let entries = self.library.entries();
                                        let all = !entries.is_empty()
                                            && entries.iter().all(|e| {
                                                self.library.selection.contains(&e.source_index)
                                            });
                                        let toggle =
                                            cx.listener(move |app, _: &ClickEvent, _, cx| {
                                                if all {
                                                    app.library.selection.clear();
                                                } else {
                                                    app.library.selection = app
                                                        .library
                                                        .entries()
                                                        .iter()
                                                        .map(|e| e.source_index)
                                                        .collect();
                                                }
                                                cx.stop_propagation();
                                                cx.notify();
                                            });
                                        el.child(
                            div()
                                .w(px(if compact { 48. } else { 36. }))
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .child(
                                    gpui_kit::base::Checkbox::new("select-all-tracks")
                                        .checked(all)
                                        .indeterminate(!all && !self.library.selection.is_empty())
                                        .accessibility_label("Select all tracks")
                                        .child(super::settings::check_indicator(
                                            if all {
                                                gpui_kit::base::CheckboxState::Checked
                                            } else if !self.library.selection.is_empty() {
                                                gpui_kit::base::CheckboxState::Indeterminate
                                            } else {
                                                gpui_kit::base::CheckboxState::Unchecked
                                            },
                                            if compact { 24. } else { 16. },
                                        ))
                                        .on_change(move |_, event, window, cx| {
                                            toggle(event, window, cx)
                                        }),
                                ),
                        )
                                    })
                                    .child(
                                        gpui_kit::base::TableHead::new(
                                            "title-column",
                                            usize::from(self.selection_mode),
                                        )
                                        .flex_basis(px(0.))
                                        .flex_grow(if compact { 1. } else { 1.4 })
                                        .min_w_0()
                                        .flex()
                                        .child(
                                            gpui_kit::base::Button::new("sort-title")
                                                .accessibility_label("Sort by title")
                                                .flex()
                                                .justify_start()
                                                .items_center()
                                                .child("Title")
                                                .child(self.sort_icon("title", compact))
                                                .gap_1()
                                                .text_color(rgb(MUTED))
                                                .flex_basis(px(0.))
                                                .flex_grow(if compact { 1. } else { 1.4 })
                                                .min_w_0()
                                                .on_click(cx.listener(
                                                    |app, event: &ClickEvent, _, cx| {
                                                        app.library.toggle_sort(
                                                            "title",
                                                            event.modifiers().shift,
                                                        );
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                    )
                                    .when(!compact, |el| {
                                        el.child(
                                            gpui_kit::base::TableHead::new(
                                                "artist-column",
                                                1 + usize::from(self.selection_mode),
                                            )
                                            .flex_basis(px(0.))
                                            .flex_grow(1.)
                                            .min_w_0()
                                            .flex()
                                            .child(
                                                gpui_kit::base::Button::new("sort-artist")
                                                    .accessibility_label("Sort by artist")
                                                    .flex()
                                                    .justify_start()
                                                    .items_center()
                                                    .child("Artist")
                                                    .child(self.sort_icon("artist", compact))
                                                    .gap_1()
                                                    .text_color(rgb(MUTED))
                                                    .flex_basis(px(0.))
                                                    .flex_grow(1.)
                                                    .min_w_0()
                                                    .on_click(cx.listener(
                                                        |app, event: &ClickEvent, _, cx| {
                                                            app.library.toggle_sort(
                                                                "artist",
                                                                event.modifiers().shift,
                                                            );
                                                            cx.notify();
                                                        },
                                                    )),
                                            ),
                                        )
                                    })
                                    .child(
                                        gpui_kit::base::TableHead::new(
                                            "duration-column",
                                            if compact { 1 } else { 2 }
                                                + usize::from(self.selection_mode),
                                        )
                                        .w(px(if compact { 48. } else { 72. }))
                                        .flex_shrink_0()
                                        .flex()
                                        .child(
                                            gpui_kit::base::Button::new("sort-duration")
                                                .accessibility_label("Sort by duration")
                                                .flex()
                                                .items_center()
                                                .child("Time")
                                                .child(self.sort_icon("duration", compact))
                                                .gap_1()
                                                .text_color(rgb(MUTED))
                                                .w(px(if compact { 48. } else { 72. }))
                                                .flex_shrink_0()
                                                .justify_end()
                                                .on_click(cx.listener(
                                                    |app, event: &ClickEvent, _, cx| {
                                                        app.library.toggle_sort(
                                                            "duration",
                                                            event.modifiers().shift,
                                                        );
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .w(px(if compact { 48. } else { 36. }))
                                            .flex_shrink_0(),
                                    ),
                            ),
                    )
                    .child(
                        gpui_kit::base::TableBody::new("track-rows")
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .child(if count == 0 {
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(muted(if total == 0 {
                                        "No tracks yet"
                                    } else {
                                        "No matching tracks"
                                    }))
                                    .into_any_element()
                            } else {
                                div()
                                    .px(px(if compact { 8. } else { 16. }))
                                    .flex_1()
                                    .min_h_0()
                                    .child(
                                        motion::Layer::scrolling(
                                            "library-tracks",
                                            count,
                                            window,
                                            cx,
                                            move |app,
                                                  range: std::ops::Range<usize>,
                                                  window,
                                                  cx| {
                                                range
                                                    .filter_map(|index| {
                                                        let entry = visible.get(index)?;
                                                        app.thumbnail(entry.track.id, cx);
                                                        let visible = visible.clone();
                                                        let style = div()
                                                            .w_full()
                                                            .h(px(if compact { 88. } else { 78. }))
                                                            .style()
                                                            .clone();
                                                        Some(
                                                            motion::Layer::cached(
                                                                ("library-track-view", index),
                                                                style,
                                                                window,
                                                                cx,
                                                                move |app, _, cx| {
                                                                    let entry = &visible[index];
                                                                    app.track_row(
                                                                        entry,
                                                                        index,
                                                                        compact,
                                                                        active_row
                                                                            == Some(
                                                                                entry.source_index,
                                                                            ),
                                                                        cx,
                                                                    )
                                                                },
                                                            )
                                                            .into_any_element(),
                                                        )
                                                    })
                                                    .collect::<Vec<_>>()
                                            },
                                        )
                                        .size_full()
                                        .flex_1()
                                        .min_h_0(),
                                    )
                                    .into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }
    fn track_row(
        &self,
        entry: &Entry,
        index: usize,
        compact: bool,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = entry.track.id;
        let source_index = entry.source_index;
        #[cfg(test)]
        if !active {
            self.inactive_row_renders.update(|count| count + 1);
        }
        let selected = self.library.selection.contains(&source_index);
        let label = title(&entry.track);
        let artist_name = artist(&entry.track);
        let view = cx.entity();
        let context_entry = entry.clone();
        let menu_entry = entry.clone();
        let menu_view = view.clone();
        let drag = TrackDrag {
            index: source_index,
            playlist: self.library.selected.clone(),
        };
        let label_drag = label.clone();
        let drag_artist = artist_name.clone();
        let drag_duration = entry.track.duration.unwrap_or(0);
        let cover = self.thumbs.get(&id).cloned().flatten();
        let sidebar = self.preferences.sidebar_width;
        let reorderable = self.library.reorderable()
            && self.library.sort.is_empty()
            && self.library.search.is_empty()
            && !self.selection_mode;
        let drag_view = cx.entity();
        let sort_drag = self.sort_drag;
        let playlist = self.library.selected.clone();
        let row = gpui_kit::base::TableRow::new(("track-row", source_index), index + 1)
            .group("track-row")
            .aria_selected(selected)
            .aria_label(format!("{} — {}", label, artist_name))
            .w_full()
            .h(px(if compact { 80. } else { 70. }))
            .px(px(if compact { 8. } else { 10. }))
            .py_2()
            .flex()
            .items_center()
            .gap(px(if compact { 8. } else { 12. }))
            .rounded(px(12.))
            .cursor_pointer()
            .focusable()
            .key_context("TrackRow")
            .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    if app.selection_mode {
                        app.library
                            .select_entry(source_index, event.keystroke.modifiers.shift);
                        cx.notify();
                    } else {
                        app.play_entries(index, None, cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .relative()
            .top(px(drag_shift(
                sort_drag,
                SortList::Tracks,
                source_index,
                if compact { 88. } else { 78. },
            )))
            .when(
                sort_drag.is_some_and(|d| d.list == SortList::Tracks && d.from == source_index),
                |el| el.opacity(0.),
            )
            .border_2()
            .border_color(gpui_kit::transparent_black())
            .when(active && !selected, |el| el.bg(rgb(0x1b252b)))
            .when(selected, |el| {
                el.border_color(rgba(0x2aabee80)).bg(rgba(0x2aabee14))
            })
            .hover(|el| el.bg(rgb(CARD)))
            .when(self.selection_mode, |el| {
                el.child(
                    div()
                        .w(px(if compact { 48. } else { 36. }))
                        .flex_shrink_0()
                        .flex()
                        .justify_center()
                        .child(
                            gpui_kit::base::Checkbox::new(("select-track", source_index))
                                .checked(selected)
                                .accessibility_label(format!("Select {}", title(&entry.track)))
                                .child(super::settings::check_mark(
                                    selected,
                                    if compact { 24. } else { 16. },
                                ))
                                .on_change({
                                    let toggle = cx.listener(move |app, _: &ClickEvent, _, cx| {
                                        app.library.select_entry(source_index, false);
                                        cx.stop_propagation();
                                        cx.notify();
                                    });
                                    move |_, event, window, cx| toggle(event, window, cx)
                                }),
                        ),
                )
            })
            .child(
                div()
                    .flex_basis(px(0.))
                    .flex_grow(if compact { 1. } else { 1.4 })
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .size(px(if compact { 56. } else { 48. }))
                            .relative()
                            .flex_shrink_0()
                            .border_1()
                            .border_color(if self.library.cached.contains(&id) {
                                rgb(PRIMARY)
                            } else {
                                rgba(0xa3a3a359)
                            })
                            .rounded(px(10.))
                            .bg(rgb(CARD))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(self.thumbs.get(&id).cloned().flatten(), |el, path| {
                                el.child(if active {
                                    if let Some(image) = self.playing_artwork.clone() {
                                        img(image).size_full().rounded(px(10.)).into_any_element()
                                    } else {
                                        img(path)
                                            .image_cache(&self.artwork_cache)
                                            .size_full()
                                            .rounded(px(10.))
                                            .opacity(0.6)
                                            .into_any_element()
                                    }
                                } else {
                                    img(path)
                                        .image_cache(&self.artwork_cache)
                                        .size_full()
                                        .rounded(px(10.))
                                        .into_any_element()
                                })
                            })
                            .when(self.thumbs.get(&id).cloned().flatten().is_none(), |el| {
                                el.child(glyph(IconName::Music, 20.).text_color(rgb(MUTED)))
                            })
                            .when_some(self.download_progress.get(&id).copied(), |el, ratio| {
                                el.child(thumbnail_progress(id, ratio))
                            })
                            .when(true, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(10.))
                                        .bg(rgba(if active { 0x00000000 } else { 0x00000073 }))
                                        .opacity(if active { 1. } else { 0. })
                                        .when(!self.selection_mode, |el| {
                                            el.when(!self.selection_mode, |el| {
                                                el.group_hover("track-row", |el| el.opacity(1.))
                                            })
                                        })
                                        .child(
                                            if active
                                                && self
                                                    .snapshot
                                                    .player
                                                    .as_ref()
                                                    .is_some_and(|p| p.is_playing)
                                            {
                                                self.track_equalizer.clone().into_any_element()
                                            } else {
                                                glyph(IconName::Play, 20.)
                                                    .text_color(rgb(if active {
                                                        PRIMARY
                                                    } else {
                                                        FG
                                                    }))
                                                    .into_any_element()
                                            },
                                        ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .id(("track-title", source_index))
                                    .test_support()
                                    .hover_title(label.clone(), &self.hover_title_epoch)
                                    .truncate()
                                    .when(active, |el| el.text_color(rgb(PRIMARY)))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_sm()
                                    .child(label),
                            )
                            .when(compact, |el| {
                                el.child(
                                    muted(artist_name.clone())
                                        .hover_title(artist_name.clone(), &self.hover_title_epoch)
                                        .text_xs()
                                        .line_height(px(16.))
                                        .truncate(),
                                )
                            }),
                    ),
            )
            .when(!compact, |el| {
                el.child(
                    div()
                        .flex_basis(px(0.))
                        .flex_grow(1.)
                        .min_w_0()
                        .id(("track-artist", source_index))
                        .test_support()
                        .child(
                            muted(artist_name.clone())
                                .truncate()
                                .hover_title(artist_name, &self.hover_title_epoch),
                        ),
                )
            })
            .child(
                muted(
                    entry
                        .track
                        .duration
                        .map(|seconds| format!("{}:{:02}", seconds / 60, seconds % 60))
                        .unwrap_or_else(|| "--:--".into()),
                )
                .w(px(if compact { 48. } else { 72. }))
                .flex_shrink_0()
                .text_right()
                .font_family(mono_font())
                .text_xs(),
            )
            .child(
                div()
                    .id(("track-options", source_index))
                    .w(px(if compact { 48. } else { 36. }))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .when(!compact, |el| el.opacity(0.))
                    .when(!self.selection_mode, |el| {
                        el.group_hover("track-row", |el| el.opacity(1.))
                    })
                    .when(!self.selection_mode, |el| {
                        el.child(
                            icon(("track-menu", source_index), IconName::Ellipsis)
                                .w(px(if compact { 36. } else { 24. }))
                                .dropdown_menu(move |menu, window, cx| {
                                    Self::track_menu(
                                        menu,
                                        &menu_view,
                                        menu_entry.clone(),
                                        window,
                                        cx,
                                    )
                                }),
                        )
                    }),
            )
            .focusable()
            .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if app.selection_mode {
                        app.library
                            .select_entry(source_index, event.keystroke.modifiers.shift);
                    } else {
                        app.play_entries(index, None, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, event: &MouseDownEvent, window, cx| {
                    if !window.last_input_was_touch() {
                        return;
                    }
                    app.touch_generation += 1;
                    let generation = app.touch_generation;
                    app.touch_press = Some((source_index, event.position, generation));
                    cx.spawn(async move |view, cx| {
                        crate::runtime::spawn(async {
                            tokio::time::sleep(Duration::from_millis(500)).await
                        })
                        .await
                        .ok();
                        let _ = view.update(cx, |app, cx| {
                            if app.touch_press.as_ref().is_some_and(|(index, _, token)| {
                                *index == source_index && *token == generation
                            }) {
                                app.touch_press = None;
                                app.suppress_touch_click = Some(source_index);
                                app.selection_mode = true;
                                app.library.select_entry(source_index, false);
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }),
            )
            .on_mouse_move(cx.listener(move |app, event: &MouseMoveEvent, _, _| {
                if let Some((index, start, _)) = app.touch_press {
                    if index == source_index
                        && ((f32::from(event.position.x - start.x)).abs() > 8.
                            || (f32::from(event.position.y - start.y)).abs() > 8.)
                    {
                        app.touch_press = None;
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|app, _, _, _| app.touch_press = None),
            )
            .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                if app.suppress_touch_click.take() == Some(source_index) {
                    return;
                }
                if app.selection_mode || event.modifiers().control || event.modifiers().platform {
                    app.selection_mode = true;
                    app.library
                        .select_entry(source_index, event.modifiers().shift);
                    cx.notify();
                } else {
                    app.play_entries(index, None, cx);
                }
            }))
            .when(reorderable, |el| {
                el.on_drag(drag, move |_, _, window, cx| {
                    let artwork_cache = drag_view.update(cx, |app, cx| {
                        app.sort_drag = Some(SortDrag {
                            list: SortList::Tracks,
                            from: source_index,
                            over: source_index,
                        });
                        cx.notify();
                        app.artwork_cache.clone()
                    });
                    let width = f32::from(window.viewport_size().width)
                        - if compact { 16. } else { sidebar + 32. };
                    let start_x = window.mouse_position().x;
                    cx.new(|_| DragPreview {
                        artwork_cache,
                        label: label_drag.clone(),
                        artist: Some(drag_artist.clone()),
                        cover: cover.clone(),
                        width,
                        height: if compact { 80. } else { 70. },
                        start_x,
                        kind: SortList::Tracks,
                        compact,
                        trailing: time(drag_duration as f64),
                    })
                })
            });
        let row = row
            .context_menu(move |menu, window, cx| {
                Self::track_menu(menu, &view, context_entry.clone(), window, cx)
            })
            .into_any_element();
        div()
            .h(px(if compact { 88. } else { 78. }))
            .w_full()
            .when(reorderable, |el| {
                el.on_drag_move(
                    cx.listener(move |app, event: &DragMoveEvent<TrackDrag>, _, cx| {
                        if !event.bounds.contains(&event.event.position) {
                            return;
                        }
                        let drag = event.drag(cx);
                        if drag.playlist == playlist
                            && app.sort_drag.is_none_or(|d| d.over != source_index)
                        {
                            app.sort_drag = Some(SortDrag {
                                list: SortList::Tracks,
                                from: drag.index,
                                over: source_index,
                            });
                            cx.notify();
                        }
                    }),
                )
                .on_drop(cx.listener(move |app, drag: &TrackDrag, _, cx| {
                    app.sort_drag = None;
                    if drag.playlist == app.library.selected {
                        app.reorder_tracks(drag.index, source_index, cx);
                    }
                }))
            })
            .child(row)
            .into_any_element()
    }
    fn track_menu(
        mut menu: PopupMenu,
        view: &Entity<Self>,
        entry: Entry,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let selected = entry.clone();
        let target = view.clone();
        menu = menu
            .min_w(px(208.))
            .max_w(px(280.))
            .group_label("Selection")
            .item(
                PopupMenuItem::new("Select")
                    .icon(menu_icon(IconName::SquareCheck))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |app, cx| {
                            app.selection_mode = true;
                            app.library.selection.insert(selected.source_index);
                            cx.notify();
                        })
                    }),
            )
            .separator()
            .group_label("Queue");
        let cached = view.read(cx).library.cached.contains(&entry.track.id);
        let liked = view.read(cx).library.liked(entry.track.id);
        let custom = matches!(view.read(cx).library.selected, PlaylistId::Custom(_));
        for (label, action, symbol) in [
            ("Play next", TrackAction::Next, IconName::ListStart),
            ("Add to end", TrackAction::Append, IconName::ListEnd),
            (
                if liked {
                    "Remove from Liked"
                } else {
                    "Add to Liked"
                },
                TrackAction::Like,
                IconName::Heart,
            ),
        ] {
            if matches!(action, TrackAction::Like) {
                menu = menu.separator().group_label("Playlist");
            }
            let target = view.clone();
            let selected = entry.clone();
            menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(symbol)).on_click(
                move |_, _, cx| {
                    target.update(cx, |app, cx| {
                        let entries = if app.selection_mode
                            && app.library.selection.contains(&selected.source_index)
                        {
                            app.library.selected_entries()
                        } else {
                            vec![selected.clone()]
                        };
                        app.track_action(action.clone(), entries, cx);
                    })
                },
            ));
        }
        let playlists = view
            .read(cx)
            .library
            .playlists
            .as_ref()
            .map(|p| p.custom.clone())
            .unwrap_or_default();
        let target = view.clone();
        let selected = entry.clone();
        menu = menu.submenu_with_icon(
            Some(menu_icon(IconName::ListPlus)),
            "Add to playlist",
            window,
            cx,
            move |mut menu, _, _| {
                let create_target = target.clone();
                let create_entry = selected.clone();
                menu = menu.item(
                    PopupMenuItem::new("Create playlist")
                        .icon(menu_icon(IconName::Plus))
                        .on_click(move |_, window, cx| {
                            create_target.update(cx, |app, cx| {
                                let tracks = if app.selection_mode
                                    && app.library.selection.contains(&create_entry.source_index)
                                {
                                    app.library
                                        .selected_entries()
                                        .iter()
                                        .map(|e| e.track.id)
                                        .collect()
                                } else {
                                    vec![create_entry.track.id]
                                };
                                app.open_panel(Panel::NewPlaylist, window, cx);
                                app.new_playlist_tracks = tracks;
                            })
                        }),
                );
                if !playlists.is_empty() {
                    menu = menu.separator();
                }
                for playlist in &playlists {
                    let target = target.clone();
                    let selected = selected.clone();
                    let id = playlist.id;
                    menu = menu.item(PopupMenuItem::new(playlist.name.clone()).on_click(
                        move |_, _, cx| {
                            target.update(cx, |app, cx| {
                                let entries = if app.selection_mode
                                    && app.library.selection.contains(&selected.source_index)
                                {
                                    app.library.selected_entries()
                                } else {
                                    vec![selected.clone()]
                                };
                                app.track_action(TrackAction::AddToPlaylist(id), entries, cx);
                            })
                        },
                    ));
                }
                menu
            },
        );
        if custom {
            let target = view.clone();
            let selected = entry.clone();
            menu = menu.item(
                PopupMenuItem::new("Remove from playlist")
                    .icon(menu_icon(IconName::ListMinus))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |app, cx| {
                            let entries = if app.selection_mode
                                && app.library.selection.contains(&selected.source_index)
                            {
                                app.library.selected_entries()
                            } else {
                                vec![selected.clone()]
                            };
                            app.track_action(TrackAction::Remove, entries, cx);
                        })
                    }),
            );
        }
        menu = menu.separator().group_label("Track");
        for (label, action, symbol) in [
            (
                if cached { "Remove from cache" } else { "Cache" },
                if cached {
                    TrackAction::RemoveCache
                } else {
                    TrackAction::Cache
                },
                if cached {
                    IconName::HardDriveUpload
                } else {
                    IconName::HardDriveDownload
                },
            ),
            ("Download", TrackAction::Export, IconName::Download),
            ("Show info", TrackAction::Info, IconName::Info),
        ] {
            let target = view.clone();
            let selected = entry.clone();
            menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(symbol)).on_click(
                move |_, _, cx| {
                    target.update(cx, |app, cx| {
                        let entries = if app.selection_mode
                            && app.library.selection.contains(&selected.source_index)
                            && !matches!(action, TrackAction::Info)
                        {
                            app.library.selected_entries()
                        } else {
                            vec![selected.clone()]
                        };
                        app.track_action(action.clone(), entries, cx);
                    })
                },
            ));
        }
        menu
    }
    pub(super) fn like_current(
        &self,
        id: &'static str,
        size: f32,
        symbol: f32,
        cx: &mut Context<Self>,
    ) -> Button {
        let current = self
            .snapshot
            .player
            .as_ref()
            .and_then(|p| p.queue.current());
        let liked = current.is_some_and(|t| self.library.liked(t.id));
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
                    .snapshot
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
    fn sort_icon(&self, column: &'static str, compact: bool) -> impl IntoElement {
        let direction = if self.library.sort == column {
            Some(self.library.descending)
        } else {
            self.library
                .additional_sort
                .iter()
                .find(|(key, _)| key == column)
                .map(|(_, desc)| *desc)
        };
        div()
            .id(SharedString::from(format!("sort-icon-{column}")))
            .test_support()
            .role(Role::Image)
            .aria_label(match direction {
                Some(false) => "Ascending",
                Some(true) => "Descending",
                None => "Unsorted",
            })
            .text_color(rgb(if direction.is_some() { FG } else { 0x606060 }))
            .child(glyph(
                match direction {
                    Some(false) => IconName::ArrowUp,
                    Some(true) => IconName::ArrowDown,
                    None => IconName::ArrowUpDown,
                },
                if compact { 24. } else { 14. },
            ))
    }
    fn bulk_actions_button(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let count = self.library.selection.len();
        Button::new("bulk-actions")
            .secondary()
            .accessibility_label(format!(
                "Actions for {count} selected {}",
                if count == 1 { "track" } else { "tracks" }
            ))
            .h(px(if compact { 48. } else { 32. }))
            .px_3()
            .gap_2()
            .when(!compact, |el| el.child("Actions"))
            .when(compact, |el| el.child(glyph(IconName::Ellipsis, 16.)))
            .child(
                div()
                    .px(px(6.))
                    .py(px(2.))
                    .rounded(px(6.))
                    .bg(rgba(0x2aabee26))
                    .text_color(rgb(PRIMARY))
                    .text_size(px(11.))
                    .font_family(mono_font())
                    .child(count.to_string()),
            )
            .dropdown_menu(move |mut menu, window, cx| {
                menu = menu.min_w(px(208.)).max_w(px(256.)).group_label("Queue");
                for (label, action, icon) in [
                    ("Play next", TrackAction::Next, IconName::ListStart),
                    ("Add to end", TrackAction::Append, IconName::ListEnd),
                ] {
                    let view = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(icon)).on_click(
                        move |_, _, cx| {
                            view.update(cx, |app, cx| {
                                app.track_action(action.clone(), app.library.selected_entries(), cx)
                            })
                        },
                    ));
                }
                let in_liked =
                    view.read(cx).library.selected == PlaylistId::Builtin("liked".into());
                let target = view.clone();
                menu = menu.separator().group_label("Playlist").item(
                    PopupMenuItem::new(if in_liked {
                        "Remove from Liked"
                    } else {
                        "Add to Liked"
                    })
                    .icon(menu_icon(IconName::Heart))
                    .on_click(move |_, _, cx| {
                        target.update(cx, |app, cx| {
                            app.track_action(
                                TrackAction::SetLiked(!in_liked),
                                app.library.selected_entries(),
                                cx,
                            )
                        })
                    }),
                );
                let playlists = view
                    .read(cx)
                    .library
                    .playlists
                    .as_ref()
                    .map(|p| p.custom.clone())
                    .unwrap_or_default();
                let target = view.clone();
                menu = menu.submenu_with_icon(
                    Some(menu_icon(IconName::ListPlus)),
                    "Add to playlist",
                    window,
                    cx,
                    move |mut menu, _, _| {
                        let create = target.clone();
                        menu = menu.item(
                            PopupMenuItem::new("Create playlist")
                                .icon(menu_icon(IconName::Plus))
                                .on_click(move |_, window, cx| {
                                    create.update(cx, |app, cx| {
                                        let tracks = app
                                            .library
                                            .selected_entries()
                                            .iter()
                                            .map(|e| e.track.id)
                                            .collect();
                                        app.open_panel(Panel::NewPlaylist, window, cx);
                                        app.new_playlist_tracks = tracks;
                                    })
                                }),
                        );
                        if !playlists.is_empty() {
                            menu = menu.separator();
                        }
                        for playlist in &playlists {
                            let target = target.clone();
                            let id = playlist.id;
                            menu = menu.item(
                                PopupMenuItem::new(playlist.name.clone())
                                    .icon(menu_icon(IconName::ListPlus))
                                    .on_click(move |_, _, cx| {
                                        target.update(cx, |app, cx| {
                                            app.track_action(
                                                TrackAction::AddToPlaylist(id),
                                                app.library.selected_entries(),
                                                cx,
                                            )
                                        })
                                    }),
                            );
                        }
                        menu
                    },
                );
                if matches!(view.read(cx).library.selected, PlaylistId::Custom(_)) {
                    let target = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new("Remove from playlist")
                            .icon(menu_icon(IconName::ListX))
                            .on_click(move |_, _, cx| {
                                target.update(cx, |app, cx| {
                                    app.track_action(
                                        TrackAction::Remove,
                                        app.library.selected_entries(),
                                        cx,
                                    )
                                })
                            }),
                    );
                }
                menu = menu.group_label("Library");
                for (label, action, icon) in [
                    ("Cache", TrackAction::Cache, IconName::HardDriveDownload),
                    ("Download", TrackAction::Export, IconName::Download),
                ] {
                    let view = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).icon(menu_icon(icon)).on_click(
                        move |_, _, cx| {
                            view.update(cx, |app, cx| {
                                app.track_action(action.clone(), app.library.selected_entries(), cx)
                            })
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }
    pub(super) fn queue_button(
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

    fn queue_button_layer(
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
                !open && sample.progress > 0. && self.snapshot.track_id.is_some(),
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
    fn filters_button(
        &self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        motion::Layer::bind(
            "filters-motion-layer",
            window,
            cx,
            move |app, window, cx| app.filters_button_layer(compact, window, cx),
        )
        .into_any_element()
    }

    fn filters_button_layer(
        &self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = "playlist-filters-popup";
        let anchor = Anchor::TopCenter;
        let offset = px(4.);
        let slide = px(-8.);
        let view = cx.entity();
        let content = view.clone();
        let size = if compact { 48. } else { 32. };
        let open = matches!(self.panel, Some(Panel::PlaylistFilters));
        let sample = motion::presence(id, open, false, window, cx);
        let popover = gpui_kit::base::Popover::new(id)
            .anchor(anchor)
            .offset(offset)
            .open(open)
            .trigger(
                icon_size(
                    "playlist-filters",
                    IconName::Funnel,
                    if compact { 24. } else { 16. },
                )
                .w(px(size))
                .h(px(size))
                .accessibility_label("Playlists filters")
                .hover_title("Playlists filters", &self.hover_title_epoch),
            )
            .on_open_change(move |open, window, cx| {
                view.update(cx, |app, cx| {
                    if *open {
                        app.open_panel(Panel::PlaylistFilters, window, cx)
                    } else if matches!(app.panel, Some(Panel::PlaylistFilters)) {
                        app.close_panel(window, cx)
                    }
                })
            })
            .content(move |_, window, cx| {
                content.update(cx, |app, cx| {
                    motion::popup_surface(app.filters_popover(window, cx), sample, slide)
                })
            });
        div()
            .relative()
            .child(popover)
            .when(!open && sample.progress > 0., |el| {
                el.child(motion::popup_exit(
                    id,
                    anchor,
                    offset,
                    motion::popup_surface(self.filters_popover(window, cx), sample, slide),
                ))
            })
            .into_any_element()
    }
    fn player_bar(
        &mut self,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let width = f32::from(window.viewport_size().width);
        let player = self.snapshot.player.as_ref();
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
                                                .map(|p| p / 100. * self.snapshot.duration_seconds)
                                                .unwrap_or(self.snapshot.current_time_seconds)
                                        ),
                                        time(self.snapshot.duration_seconds)
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
    pub(super) fn playback_controls(
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
        let player = self.snapshot.player.as_ref();
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
    pub(super) fn playback_progress(
        &self,
        location: &'static str,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        let disabled = disabled
            || !self.snapshot.duration_seconds.is_finite()
            || self.snapshot.duration_seconds <= 0.;
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
                        app.seek_preview =
                            Some((fraction, fraction as f64 * app.snapshot.duration_seconds));
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
                                    .children(self.snapshot.buffered_ranges.iter().map(|range| {
                                        let duration = self.snapshot.duration_seconds.max(1.);
                                        div()
                                            .absolute()
                                            .top(px(16.))
                                            .h(px(4.))
                                            .left(relative(
                                                (range.start / duration).clamp(0., 1.) as f32
                                            ))
                                            .w(relative(
                                                ((range.end - range.start) / duration).clamp(0., 1.)
                                                    as f32,
                                            ))
                                            .bg(rgba(0xffffff4d))
                                    }))
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
    fn seek_loading_stripes(started: std::time::Instant, window: &Window, cx: &App) -> AnyElement {
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
                        window.paint_quad(fill(bounds, rgba(0x2aabee1f)));
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
                            window.paint_path(path, rgba(0x2aabee59));
                        }
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }

    pub(super) fn reset_volume_hover(&mut self) {
        self.volume_hover = false;
        self.volume_exact_open = false;
        self.volume_pointer_over = [false; 2];
        self.volume_dragging = false;
        self.volume_close_task = None;
    }

    pub(super) fn update_volume_hover(&mut self, window: &Window, cx: &mut Context<Self>) {
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

    fn volume_control(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        motion::Layer::bind("volume-motion-layer", window, cx, move |app, window, cx| {
            app.volume_control_layer(window, cx)
        })
        .into_any_element()
    }

    fn volume_control_layer(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
    fn finish_volume_drag(&mut self, window: &Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.volume_dragging) {
            self.update_volume_hover(window, cx);
            cx.notify();
        }
    }
    fn volume_popup(&self, cx: &mut Context<Self>) -> AnyElement {
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
                                    .bg(rgb(0x272727)),
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
    fn fullscreen_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let path = self.palette_path.clone();
        let frame = artwork::FullscreenFrame {
            track_id: self.snapshot.track_id,
            cover: path,
            image: self.fullscreen_cover.clone(),
            backdrop: self.fullscreen_backdrop.clone(),
            palette: self.palette,
            settings: self.preferences.bounce.clone(),
            profile: self.bounce_profile.clone(),
            profile_ready_at: self.bounce_ready_at,
            seconds: self.snapshot.current_time_seconds,
            playing: self.snapshot.status == audio::Status::Playing,
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

    fn fullscreen_controls_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let width = f32::from(window.viewport_size().width);
        let track = self
            .snapshot
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
                if let Some(player) = &self.snapshot.player {
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
                                                        p / 100. * self.snapshot.duration_seconds
                                                    })
                                                    .unwrap_or(self.snapshot.current_time_seconds),
                                            ))),
                                        )
                                        .child(self.playback_controls(false, false, width, cx))
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .justify_end()
                                                .child(muted(time(self.snapshot.duration_seconds))),
                                        ),
                                ),
                        )
                        .when(!controls_visible, |el| el.child(motion::exit_shield())),
                )
            })
            .into_any_element()
    }
    fn search_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let entries = self.library.track_list().entries;
        let count = entries.len();
        let active_row = self.library.playing_source_index(&self.snapshot);
        let inset = if cfg!(target_os = "macos") && !window.is_fullscreen() {
            32.
        } else {
            0.
        };
        div()
            .id("compact-search-overlay")
            .test_support()
            .absolute()
            .top(px(inset))
            .left_0()
            .right_0()
            .bottom(px(if self.snapshot.track_id.is_some() {
                96.
            } else {
                0.
            }))
            .bg(rgb(CARD))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        icon_size("close-track-search", IconName::ArrowLeft, 20.)
                            .hover_title("Close track search", &self.hover_title_epoch)
                            .on_click(
                                cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                            ),
                    )
                    .child(
                        Input::new(&self.inputs.search)
                            .cleanable(true)
                            .id("compact-search-input")
                            .aria_label("Search tracks")
                            .appearance(false)
                            .text_size(px(16.))
                            .flex_1()
                            .min_w_0(),
                    ),
            )
            .child(div().flex_shrink_0().px_5().py_3().child(muted(format!(
                "{count} {} in {}",
                if count == 1 { "track" } else { "tracks" },
                self.library.name()
            ))))
            .child(if count == 0 {
                div()
                    .px_5()
                    .py_6()
                    .child(muted("No tracks found."))
                    .into_any_element()
            } else {
                motion::Layer::scrolling(
                    "search-results",
                    count,
                    window,
                    cx,
                    move |app, range: std::ops::Range<usize>, _, cx| {
                        range
                            .filter_map(|index| {
                                let entry = entries.get(index)?;
                                app.thumbnail(entry.track.id, cx);
                                Some(app.track_row(
                                    entry,
                                    index,
                                    true,
                                    active_row == Some(entry.source_index),
                                    cx,
                                ))
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .flex_1()
                .min_h_0()
                .w_full()
                .px_2()
                .into_any_element()
            })
            .into_any_element()
    }
    fn panel_view(
        &mut self,
        panel: Panel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match panel {
            Panel::Navigation => return self.navigation_drawer(window, cx),
            Panel::Search => return self.search_overlay(window, cx),
            Panel::Queue => return div().into_any_element(),
            Panel::PlaylistFilters => return div().into_any_element(),
            _ => {}
        }
        if matches!(
            panel,
            Panel::NewPlaylist | Panel::RenamePlaylist(_) | Panel::SaveQueue
        ) {
            return self.playlist_dialog(&panel, window, cx);
        }
        if let Panel::TrackInfo(id) = panel {
            return self.track_info_dialog(id, window, cx);
        }
        if matches!(panel, Panel::Logs) {
            return self.logs_dialog(window, cx);
        }
        if matches!(panel, Panel::DownloadResult) {
            return self.download_dialog(window, cx);
        }
        let panel_title = match &panel {
            Panel::Navigation => "SoundGrammy",
            Panel::About => "About SoundGrammy",
            Panel::PlaylistFilters => "Playlists filters",
            Panel::Volume => "Volume",
            Panel::Search => "Search tracks",
            Panel::Settings => "Settings",
            Panel::Proxy => "Connection settings",
            Panel::Queue => "Queue",
            Panel::NewPlaylist => "New playlist",
            Panel::RenamePlaylist(_) => "Rename playlist",
            Panel::SaveQueue => "Save queue as playlist",
            Panel::TrackInfo(_) => "Track information",
            Panel::Logs => "Logs",
            Panel::DownloadResult => "Download finished",
            Panel::CompactPlayer => "Now playing",
            Panel::Confirm(_) => "Confirm",
            Panel::Import { .. } => "Import playlist",
        };
        let settings = matches!(panel, Panel::Settings);
        let content = match panel {
            Panel::Navigation
            | Panel::PlaylistFilters
            | Panel::Queue
            | Panel::NewPlaylist
            | Panel::RenamePlaylist(_)
            | Panel::SaveQueue
            | Panel::TrackInfo(_)
            | Panel::Logs
            | Panel::DownloadResult
            | Panel::CompactPlayer => unreachable!("specialized overlay handled above"),
            Panel::About => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(heading("SoundGrammy"))
                .child(muted(format!("Version {}", env!("CARGO_PKG_VERSION"))))
                .child("A local-first player for your Telegram profile music.")
                .child(muted("Copyright © 2026 Rasten Remizov · GPL-3.0-only"))
                .into_any_element(),
            Panel::Search => unreachable!("specialized search overlay handled above"),
            Panel::Volume => div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    button(
                        "mute-volume",
                        if self.preferences.volume == 0. {
                            "Unmute"
                        } else {
                            "Mute"
                        },
                    )
                    .on_click(cx.listener(|app, _, _, cx| app.mute(cx))),
                )
                .child(
                    Input::new(&self.inputs.exact_volume)
                        .aria_label("Exact volume percent")
                        .w(px(100.)),
                )
                .child(
                    Slider::new(&self.volume_slider)
                        .accessibility_label("Volume")
                        .flex_1(),
                )
                .into_any_element(),
            Panel::Settings => self.settings_view(window, cx),
            Panel::Proxy => self.proxy_view(cx),
            Panel::Confirm(action) => {
                let text=match action{Confirmation::Logout=>"Log out of your Telegram account?",Confirmation::FullReindex=>"Reindex your entire saved music library? Tracks will be hidden until indexing finishes.",Confirmation::ClearCache=>"Clear all cached audio?",Confirmation::ClearStatistics=>"Permanently clear local listening statistics?",Confirmation::DeletePlaylist(_)=>"Delete this playlist? Your Telegram music will be kept.",Confirmation::DisconnectLastFm=>"Disconnect Last.fm? Choose whether to retain or delete pending scrobbles."};
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(text)
                    .when(matches!(action, Confirmation::DisconnectLastFm), |el| {
                        el.child(
                            button(
                                "disconnect-delete",
                                "Delete pending scrobbles and disconnect",
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.panel = app.panel_stack.pop();
                                app.lastfm_action("disconnect-delete", cx);
                            })),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(button("cancel-confirm", "Cancel").on_click(
                                cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                            ))
                            .child(
                                Button::new("confirm")
                                    .primary()
                                    .label(if matches!(action, Confirmation::DisconnectLastFm) {
                                        "Retain and disconnect"
                                    } else {
                                        "Confirm"
                                    })
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.confirm(action.clone(), cx)
                                    })),
                            ),
                    )
                    .into_any_element()
            }
            Panel::Import { path, preview } => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(heading(preview.suggested_name.clone()))
                .child(
                    muted(
                        std::path::Path::new(&path)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.clone()),
                    )
                    .id("playlist-import-path")
                    .test_support()
                    .truncate()
                    .hover_title(path.clone(), &self.hover_title_epoch),
                )
                .child(Input::new(&self.inputs.name).aria_label("Imported playlist name"))
                .child(muted(format!(
                    "{} tracks available · {} unavailable",
                    preview.succeeded.len(),
                    preview.failed.len()
                )))
                .children(preview.failed.iter().map(|entry| {
                    muted(format!(
                        "{} — {}",
                        entry.title.as_deref().unwrap_or("Unknown title"),
                        entry.reason
                    ))
                }))
                .child(
                    Button::new("confirm-import")
                        .primary()
                        .label("Import playlist")
                        .on_click(cx.listener(move |app, _, _, cx| {
                            let path = path.clone();
                            let name = app.inputs.name.read(cx).value().trim().to_owned();
                            let name = (!name.is_empty()).then_some(name);
                            app.mutation(
                                cx,
                                move |host| async move {
                                    commands::playlists::import_playlist_json(
                                        &host.state::<AppState>(),
                                        path,
                                        name,
                                    )
                                    .await
                                },
                                |app, result, cx| {
                                    app.panel = None;
                                    app.notice = Some(format!(
                                        "Imported {} tracks into {} · {} unavailable",
                                        result.succeeded.len(),
                                        result.playlist_name,
                                        result.failed.len()
                                    ));
                                    app.refresh_playlists(cx);
                                },
                            );
                        })),
                )
                .into_any_element(),
        };
        let popup =
            div()
                .id("modal")
                .test_support()
                .track_focus(&self.modal_focus)
                .role(Role::Group)
                .aria_label(panel_title)
                .w(px(if settings { 512. } else { 448. }))
                .max_w_full()
                .max_h(px((f32::from(window.viewport_size().height)
                    - if cfg!(target_os = "macos") && !window.is_fullscreen() {
                        32.
                    } else {
                        0.
                    })
                    * 0.9))
                .flex()
                .flex_col()
                .rounded(px(16.))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(CARD))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .px_4()
                        .py_2()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(heading(panel_title).flex_1())
                        .child(icon("close-panel", IconName::X).on_click(
                            cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                        )),
                )
                .child(
                    div()
                        .id("panel-scroll")
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_4()
                        .child(content),
                )
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .into_any_element();
        self.modal_surface(popup, false, window, cx)
    }
    pub(super) fn queue_view(&mut self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let session = self.snapshot.player.as_ref();
        let queue = session.map(|s| s.queue.clone()).unwrap_or_default();
        let revision = session.map(|s| s.revision).unwrap_or(0);
        let next = queue_items(
            queue.tracks.len(),
            queue.cursor,
            self.preferences.history_open,
            self.preferences.up_next_open,
        );
        if self.queue_items != next {
            self.queue_list.reset(next.len());
            self.queue_items = next;
        }
        let items = Arc::new(self.queue_items.clone());
        div()
            .flex()
            .flex_col()
            .min_h_0()
            .when(queue.tracks.is_empty(), |el| {
                el.child(
                    div()
                        .h(px(160.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(muted("Queue is empty")),
                )
            })
            .child(
                list(
                    self.queue_list.clone(),
                    cx.processor(move |app, item: usize, _, cx| match &items[item] {
                        QueueItem::Header(section, count) => {
                            let section = *section;
                            let count = *count;
                            let open = match section {
                                "History" => app.preferences.history_open,
                                "Up next" => app.preferences.up_next_open,
                                _ => true,
                            };
                            div()
                                .id(SharedString::from(format!("queue-section-{section}")))
                                .test_support()
                                .role(if section == "Now playing" {
                                    Role::Label
                                } else {
                                    Role::Button
                                })
                                .aria_label(if section == "Now playing" {
                                    "Now playing".to_owned()
                                } else {
                                    format!("{section} {count}")
                                })
                                .h(px(if compact { 48. } else { 28. }))
                                .w_full()
                                .px_2()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_size(px(10.))
                                .line_height(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(MUTED))
                                .when(section != "Now playing", |el| {
                                    el.cursor_pointer().hover(|el| el.bg(rgb(0x272727))).child(
                                        glyph(
                                            if open {
                                                IconName::ChevronDown
                                            } else {
                                                IconName::ChevronRight
                                            },
                                            12.,
                                        ),
                                    )
                                })
                                .child(div().flex_1().child(section.to_uppercase()))
                                .when(section != "Now playing", |el| {
                                    el.child(
                                        div().font_family(mono_font()).child(count.to_string()),
                                    )
                                })
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    match section {
                                        "History" => {
                                            app.preferences.history_open =
                                                !app.preferences.history_open
                                        }
                                        "Up next" => {
                                            app.preferences.up_next_open =
                                                !app.preferences.up_next_open
                                        }
                                        _ => {}
                                    }
                                    app.save_preferences();
                                    cx.notify();
                                }))
                                .into_any_element()
                        }
                        QueueItem::Track(index) => {
                            let index = *index;
                            let track = queue.tracks[index].clone();
                            app.thumbnail(track.id, cx);
                            let label = title(&track);
                            let drag_label = label.clone();
                            let drag_view = cx.entity();
                            let row = div()
                                .id(("queue-track", index))
                                .relative()
                                .top(px(drag_shift(
                                    app.sort_drag,
                                    SortList::Queue,
                                    index,
                                    if compact { 64. } else { 56. },
                                )))
                                .when(
                                    app.sort_drag.is_some_and(|d| {
                                        d.list == SortList::Queue && d.from == index
                                    }),
                                    |el| el.opacity(0.),
                                )
                                .test_support()
                                .group("queue-track")
                                .h(px(if compact { 64. } else { 56. }))
                                .w_full()
                                .p_1()
                                .rounded(px(12.))
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(index as i64 == queue.cursor, |el| el.bg(rgb(0x203039)))
                                .child(
                                    div()
                                        .size(px(48.))
                                        .flex_shrink_0()
                                        .rounded(px(10.))
                                        .overflow_hidden()
                                        .bg(rgb(BG))
                                        .when(app.library.cached.contains(&track.id), |el| {
                                            el.border_1().border_color(rgb(PRIMARY))
                                        })
                                        .when_some(
                                            app.thumbs.get(&track.id).cloned().flatten(),
                                            |el, path| {
                                                el.child(
                                                    img(path)
                                                        .image_cache(&app.artwork_cache)
                                                        .size_full()
                                                        .object_fit(ObjectFit::Cover)
                                                        .rounded(px(8.)),
                                                )
                                            },
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .id(("queue-title", index))
                                                .test_support()
                                                .hover_title(label.clone(), &app.hover_title_epoch)
                                                .truncate()
                                                .when(index as i64 == queue.cursor, |el| {
                                                    el.text_color(rgb(PRIMARY))
                                                })
                                                .child(label),
                                        )
                                        .child(
                                            muted(artist(&track)).text_xs().truncate().hover_title(
                                                artist(&track),
                                                &app.hover_title_epoch,
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .opacity(0.)
                                        .group_hover("queue-track", |el| el.opacity(1.))
                                        .child(
                                            icon(("queue-remove", index), IconName::X).on_click(
                                                cx.listener(move |app, _, _, cx| {
                                                    cx.stop_propagation();
                                                    app.command(
                                                        Command::Remove {
                                                            indices: vec![index as i64],
                                                            queue_revision: revision,
                                                        },
                                                        cx,
                                                    );
                                                }),
                                            ),
                                        ),
                                )
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.command(
                                        Command::Jump {
                                            index: index as i64,
                                            queue_revision: revision,
                                        },
                                        cx,
                                    )
                                }))
                                .on_drag(QueueDrag { index, revision }, {
                                    let performer = artist(&track);
                                    let cover = app.thumbs.get(&track.id).cloned().flatten();
                                    move |_, _, window, cx| {
                                        let artwork_cache = drag_view.update(cx, |app, cx| {
                                            app.sort_drag = Some(SortDrag {
                                                list: SortList::Queue,
                                                from: index,
                                                over: index,
                                            });
                                            cx.notify();
                                            app.artwork_cache.clone()
                                        });
                                        let start_x = window.mouse_position().x;
                                        cx.new(|_| DragPreview {
                                            artwork_cache,
                                            label: drag_label.clone(),
                                            artist: Some(performer.clone()),
                                            cover: cover.clone(),
                                            width: 312.,
                                            height: if compact { 64. } else { 56. },
                                            start_x,
                                            kind: SortList::Queue,
                                            compact,
                                            trailing: String::new(),
                                        })
                                    }
                                })
                                .into_any_element();
                            div()
                                .h(px(if compact { 64. } else { 56. }))
                                .w_full()
                                .on_drag_move(cx.listener(
                                    move |app, event: &DragMoveEvent<QueueDrag>, _, cx| {
                                        if !event.bounds.contains(&event.event.position) {
                                            return;
                                        }
                                        let drag = event.drag(cx);
                                        if drag.revision == revision
                                            && app.sort_drag.is_none_or(|d| d.over != index)
                                        {
                                            app.sort_drag = Some(SortDrag {
                                                list: SortList::Queue,
                                                from: drag.index,
                                                over: index,
                                            });
                                            cx.notify();
                                        }
                                    },
                                ))
                                .on_drop(cx.listener(move |app, drag: &QueueDrag, _, cx| {
                                    app.sort_drag = None;
                                    if drag.revision == revision {
                                        app.command(
                                            Command::Reorder {
                                                from_index: drag.index as i64,
                                                to_index: index as i64,
                                                queue_revision: revision,
                                            },
                                            cx,
                                        );
                                    }
                                }))
                                .child(row)
                                .into_any_element()
                        }
                    }),
                )
                .h(px(if self.queue_items.is_empty() {
                    0.
                } else {
                    392.
                }))
                .px_1()
                .py_2()
                .w_full(),
            )
            .into_any_element()
    }
    fn load_bounce(&mut self, id: i64, cx: &mut Context<Self>) {
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
                if app.bounce_track != Some(id) || app.snapshot.track_id != Some(id) {
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
