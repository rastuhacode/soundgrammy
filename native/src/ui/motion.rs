//! SoundGrammy's motion styling around GPUI Base Kit interaction primitives.
use super::*;
use gpui_kit::base::{
    animation::cubic_bezier,
    motion::{Presence, PresencePhase, PresenceSample, Transition},
    Popup,
};

type RenderLayer = dyn Fn(&mut PlayerApp, &mut Window, &mut Context<PlayerApp>) -> AnyElement;

// The renderer borrows the player for data and event handlers. Frame requests
// still belong to this view, so unrelated cached siblings can reuse their work.
pub(super) struct Layer {
    player: WeakEntity<PlayerApp>,
    render: Box<RenderLayer>,
    _player_updates: Option<Subscription>,
    model_updates: Option<(EntityId, Subscription)>,
    scroll: Option<ScrollMotion>,
}

impl Layer {
    pub(super) fn bind(
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut Context<PlayerApp>,
        render: impl Fn(&mut PlayerApp, &mut Window, &mut Context<PlayerApp>) -> AnyElement + 'static,
    ) -> Entity<Self> {
        let layer = Self::for_element(id, window, cx, false);
        layer.update(cx, |layer, _| layer.render = Box::new(render));
        layer
    }

    pub(super) fn cached<M: 'static, K: PartialEq + Clone + 'static>(
        id: impl Into<ElementId>,
        style: StyleRefinement,
        model: &Entity<M>,
        inputs: K,
        window: &mut Window,
        cx: &mut Context<PlayerApp>,
        render: impl Fn(&mut PlayerApp, &mut Window, &mut Context<PlayerApp>) -> AnyElement + 'static,
    ) -> impl IntoElement {
        // An active screen reader needs a complete accessibility tree each frame.
        let cache = !window.is_a11y_active();
        let id = id.into();
        let remembered =
            window.use_keyed_state((id.clone(), "render-inputs"), cx, |_, _| None::<K>);
        let changed = remembered.read(cx).as_ref() != Some(&inputs);
        remembered.update(cx, |value, _| *value = Some(inputs));
        // Model notifications invalidate this feature. Shell-only changes use
        // explicit presentation inputs rather than observing the entire app.
        let layer = Self::for_element(id, window, cx, false);
        layer.update(cx, |layer, cx| {
            if layer.model_updates.as_ref().map(|(id, _)| *id) != Some(model.entity_id()) {
                layer.model_updates =
                    Some((model.entity_id(), cx.observe(model, |_, _, cx| cx.notify())));
            }
            layer.render = Box::new(render);
            if changed {
                cx.notify();
            }
        });
        if cache {
            layer.cached(style).into_any_element()
        } else {
            uncached_layer(layer, style)
        }
    }

    /// Resample precise pointer deltas at display cadence. Native momentum stays
    /// native; interpolation spans at most one input interval (not another fling).
    pub(super) fn scrolling(
        id: &'static str,
        count: usize,
        window: &mut Window,
        cx: &mut Context<PlayerApp>,
        render: impl Fn(
                &mut PlayerApp,
                std::ops::Range<usize>,
                &mut Window,
                &mut Context<PlayerApp>,
            ) -> Vec<AnyElement>
            + 'static,
    ) -> Div {
        let layer = Self::for_element((ElementId::from(id), "smooth-view"), window, cx, true);
        let render = std::rc::Rc::new(render);
        layer.update(cx, |layer, _| {
            let handle = layer
                .scroll
                .get_or_insert_with(ScrollMotion::default)
                .handle
                .clone();
            layer.render = Box::new(move |_, _, cx| {
                let render = render.clone();
                uniform_list(
                    id,
                    count,
                    cx.processor(move |app, range, window, cx| render(app, range, window, cx)),
                )
                .track_scroll(&handle)
                .size_full()
                .flex_1()
                .min_h_0()
                .into_any_element()
            });
        });
        div().size_full().flex_1().min_h_0().child(layer)
    }

    fn for_element(
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut Context<PlayerApp>,
        cached: bool,
    ) -> Entity<Self> {
        let player = cx.entity();
        // Key the handle rather than the view itself. Keyed state forwards
        // notifications to its owner, which would undo the animation boundary.
        let handle = window.use_keyed_state(id, cx, |_, cx| {
            cx.new(|cx: &mut Context<Self>| Self {
                player: player.downgrade(),
                render: Box::new(|_, _, _| Empty.into_any_element()),
                _player_updates: cached.then(|| cx.observe(&player, |_, _, cx| cx.notify())),
                model_updates: None,
                scroll: None,
            })
        });
        handle.read(cx).clone()
    }
}

impl Render for Layer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(scroll) = &mut self.scroll {
            if scroll.dragging.get() {
                scroll.active = false;
            }
            scroll.advance(cx.background_executor().now());
            if scroll.active {
                scroll.request_frame(window, cx);
            }
        }
        let content = self
            .player
            .update(cx, |player, cx| (self.render)(player, window, cx))
            .unwrap_or_else(|_| Empty.into_any_element());
        if self.scroll.is_none() {
            return content;
        }
        let view = cx.weak_entity();
        let scroll = self.scroll.as_ref().unwrap();
        let scrollbar = gpui_kit::base::Scrollbar::vertical(&ScrollThumbHandle {
            handle: scroll.handle.clone(),
            dragging: scroll.dragging.clone(),
        })
        .id("track-scrollbar")
        .mode(gpui_kit::base::ScrollbarMode::Scrolling)
        .styles(|style| {
            style
                .track(|track| track.width(px(10.)))
                .thumb(|thumb| {
                    thumb
                        .width(px(6.))
                        .inset(px(1.))
                        .radius(px(4.))
                        .bg(tint(MUTED, 0x99))
                })
                .thumb_hover(|thumb| thumb.width(px(8.)).bg(tint(MUTED, 0xcc)))
                .thumb_active(|thumb| thumb.width(px(8.)).bg(rgb(MUTED)))
        });
        div()
            .relative()
            .size_full()
            .child(content)
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |_, hitbox, window, _| {
                        window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture || !hitbox.is_hovered(window) {
                                return;
                            }
                            let _ = view.update(cx, |layer, cx| {
                                let Some(scroll) = &mut layer.scroll else {
                                    return;
                                };
                                if cx.reduce_motion()
                                    || super::preview::direct_pointer_scroll()
                                    || event.source != ScrollSource::Pointer
                                    || !event.delta.precise()
                                {
                                    scroll.active = false;
                                    return;
                                }
                                let delta = event.delta.pixel_delta(px(20.));
                                if delta.y == px(0.) || delta.x.abs() > delta.y.abs() {
                                    return;
                                }
                                if scroll.input(delta.y, cx.background_executor().now()) {
                                    cx.stop_propagation();
                                    scroll.request_frame(window, cx);
                                }
                            });
                        })
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(scrollbar)
            .into_any_element()
    }
}

#[derive(Default)]
struct ScrollMotion {
    handle: UniformListScrollHandle,
    dragging: std::rc::Rc<std::cell::Cell<bool>>,
    from: f32,
    target: f32,
    started: Option<std::time::Instant>,
    last_input: Option<std::time::Instant>,
    duration: f32,
    active: bool,
    frame_pending: bool,
}
impl ScrollMotion {
    fn advance(&mut self, now: std::time::Instant) {
        if !self.active {
            return;
        }
        let handle = &self.handle.0.borrow().base_handle;
        let max = f32::from(handle.max_offset().y).max(0.);
        self.target = self.target.clamp(-max, 0.);
        let progress = self
            .started
            .map_or(1., |start| {
                now.saturating_duration_since(start).as_secs_f32() / self.duration.max(0.001)
            })
            .clamp(0., 1.);
        let y = self.from + (self.target - self.from) * progress;
        handle.set_offset(point(handle.offset().x, px(y.clamp(-max, 0.))));
        if progress >= 1. {
            self.active = false;
        }
    }
    fn input(&mut self, delta: Pixels, now: std::time::Instant) -> bool {
        self.advance(now);
        let handle = &self.handle.0.borrow().base_handle;
        let current = f32::from(handle.offset().y);
        let target = (if self.active { self.target } else { current }) + f32::from(delta);
        let target = target.clamp(-f32::from(handle.max_offset().y).max(0.), 0.);
        if target == current {
            return false;
        }
        self.duration = self.last_input.map_or(1. / 60., |last| {
            now.saturating_duration_since(last)
                .as_secs_f32()
                .clamp(1. / 240., 1. / 60.)
        });
        self.from = current;
        self.target = target;
        self.started = Some(now);
        self.last_input = Some(now);
        self.active = true;
        true
    }
    fn request_frame(&mut self, window: &Window, cx: &mut Context<Layer>) {
        if self.frame_pending {
            return;
        }
        self.frame_pending = true;
        let view = cx.weak_entity();
        window.on_next_frame(move |_, cx| {
            let _ = view.update(cx, |layer, cx| {
                if let Some(scroll) = &mut layer.scroll {
                    scroll.frame_pending = false;
                }
                cx.notify();
            });
        });
    }
}

fn uncached_layer(layer: impl IntoElement, style: StyleRefinement) -> AnyElement {
    let mut wrapper = div();
    wrapper.style().refine(&style);
    wrapper.child(layer).into_any_element()
}

pub(super) struct Equalizer {
    login: bool,
    started: Option<std::time::Instant>,
    next_tick: Option<Task<()>>,
}

impl Equalizer {
    pub(super) fn new(login: bool) -> Self {
        Self {
            login,
            started: None,
            next_tick: None,
        }
    }

    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        if self.next_tick.is_none() {
            // These small, decorative bars do not need one full-window frame
            // for each refresh of a 60 Hz or 120 Hz display.
            let wait = cx
                .background_executor()
                .timer(Duration::from_nanos(33_333_334));
            self.next_tick = Some(cx.spawn(async move |view, cx| {
                wait.await;
                let _ = view.update(cx, |equalizer, cx| {
                    equalizer.next_tick = None;
                    cx.notify();
                });
            }));
        }
    }
}

impl Render for Equalizer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = cx.background_executor().now();
        let started = *self.started.get_or_insert(now);
        let seconds = now.saturating_duration_since(started).as_secs_f32() % 1.7;
        let animate = window.is_visible() && !cx.reduce_motion();
        let height = if self.login { 28. } else { 16. };
        let width = if self.login { 4. } else { 3. };
        let delays = [0., 0.18, 0.36, 0.12];
        let initial_heights = [0.4, 0.75, 0.55, 0.85];
        let count = if self.login { 4 } else { 3 };
        div()
            .h(px(height))
            .flex()
            .items_end()
            .justify_center()
            .gap(px(2.))
            .when(animate, |el| {
                // A virtual list can measure an offscreen row without painting it.
                let equalizer = cx.weak_entity();
                el.child(
                    canvas(
                        |_, _, _| (),
                        move |_, _, _, cx| {
                            let _ =
                                equalizer.update(cx, |equalizer, cx| equalizer.schedule_tick(cx));
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
            .children((0..count).map(|index| {
                let phase = (seconds - delays[index]) / 0.85;
                let fraction = if !animate {
                    initial_heights[index]
                } else if self.login {
                    0.18 + 0.74 * (0.5 - 0.5 * (phase * std::f32::consts::PI).cos())
                } else {
                    let cycle = phase.rem_euclid(2.);
                    let triangle = 1. - (1. - cycle).abs();
                    0.18 + 0.74 * cubic_bezier(0.42, 0., 0.58, 1.)(triangle)
                };
                div()
                    .id((
                        if self.login {
                            "login-equalizer-bar"
                        } else {
                            "equalizer-bar"
                        },
                        index,
                    ))
                    .test_support()
                    .w(px(width))
                    .h(px(height * fraction))
                    .rounded(px(2.))
                    .bg(rgb(views::PRIMARY))
            }))
    }
}

pub(super) fn seek_tooltip(label: String) -> AnyElement {
    gpui_kit::base::Tooltip::new("seek-preview-tooltip")
        .text_xs()
        .font_family(mono_font())
        .text_color(rgb(views::FG))
        .child(div().id("seek-tooltip").test_support().child(label))
        .with_animation(
            "seek-tooltip-enter",
            Animation::new(Duration::from_millis(150))
                .with_easing(gpui_kit::base::animation::ease_out_cubic),
            |tooltip, opacity| tooltip.opacity(opacity),
        )
        .into_any_element()
}

pub(super) fn presence(
    id: impl Into<ElementId>,
    open: bool,
    drawer: bool,
    window: &mut Window,
    cx: &mut App,
) -> PresenceSample {
    Presence::new(id.into(), open)
        .transition(if drawer {
            Transition::new(Duration::from_millis(if open { 450 } else { 400 }))
                .ease(cubic_bezier(0.22, 1., 0.36, 1.))
        } else {
            Transition::new(Duration::from_millis(100)).ease(cubic_bezier(0.25, 0.1, 0.25, 1.))
        })
        .sample(window, cx)
}

pub(super) fn fullscreen_controls(
    id: &'static str,
    visible: bool,
    milliseconds: u64,
    window: &mut Window,
    cx: &mut App,
) -> PresenceSample {
    Presence::new(id, visible)
        .transition(
            Transition::new(Duration::from_millis(milliseconds))
                .ease(cubic_bezier(0.4, 0., 0.2, 1.)),
        )
        .sample(window, cx)
}

pub(super) fn popup_surface(
    content: impl IntoElement,
    sample: PresenceSample,
    slide: Pixels,
) -> Div {
    let progress = sample.progress;
    div()
        .relative()
        .top(slide * (1. - progress))
        .opacity(progress)
        .child(content)
        .when(sample.phase == PresencePhase::Exiting, |el| {
            el.child(exit_shield())
        })
}

// Keep the closed popup's paint at the same Base Kit anchor, without retaining
// its live Popover/HoverCard state, focus handling, or dismissal listeners.
pub(super) fn popup_exit(id: &'static str, anchor: Anchor, offset: Pixels, content: Div) -> Div {
    div().absolute().inset_0().child(
        Popup::new((id, 1_usize), div().size_full())
            .size_full()
            .anchor(anchor)
            .offset(offset)
            .content(content),
    )
}

pub(super) fn exit_shield() -> Div {
    div()
        .absolute()
        .inset_0()
        .occlude()
        .on_scroll(|_, _, cx| cx.stop_propagation())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use gpui_kit::test::TestWindowExt;
    #[gpui_kit::test]
    fn uncached_layers_preserve_the_same_layout_constraints(cx: &mut TestAppContext) {
        struct Probe;
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(div().flex_1())
                    .child(uncached_layer(
                        div().id("uncached-content").test_support().size_full(),
                        div().w_full().h(px(96.)).flex_shrink_0().style().clone(),
                    ))
            }
        }
        let window = cx.update(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(1200.), px(800.)),
                    ))),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Probe),
            )
            .unwrap()
        });
        cx.update_window(*window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("uncached-content").bounds().size,
                size(px(1200.), px(96.))
            );
        })
        .unwrap();
    }
}

#[derive(Clone)]
struct ScrollThumbHandle {
    handle: UniformListScrollHandle,
    dragging: std::rc::Rc<std::cell::Cell<bool>>,
}
impl gpui_kit::base::ScrollbarHandle for ScrollThumbHandle {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.handle.0.borrow().base_handle.bounds()
    }
    fn offset(&self) -> Point<Pixels> {
        self.handle.0.borrow().base_handle.offset()
    }
    fn set_offset(&self, offset: Point<Pixels>) {
        self.handle.0.borrow().base_handle.set_offset(offset);
    }
    fn content_size(&self) -> Size<Pixels> {
        let state = self.handle.0.borrow();
        let handle = &state.base_handle;
        (handle.max_offset() + handle.bounds().size.into()).into()
    }
    fn start_drag(&self) {
        self.dragging.set(true);
    }
    fn end_drag(&self) {
        self.dragging.set(false);
    }
}
