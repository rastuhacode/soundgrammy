//! Retained sign-in feature: native inputs, focus and QR presentation have
//! their own lifetime. Account/session service calls stay in the coordinator.
use super::views::{button, glyph, muted};
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    Disableable,
};

#[derive(Clone)]
pub(super) struct LoginInputs {
    pub phone: Entity<InputState>,
    pub code: Entity<InputState>,
    pub password: Entity<InputState>,
}
#[derive(Default, PartialEq)]
struct LoginProps {
    busy: bool,
    error: Option<String>,
    proxy_enabled: bool,
}
impl LoginProps {
    fn from_app(app: &PlayerApp, cx: &App) -> Self {
        Self {
            busy: app.busy > 0,
            error: app.error.clone(),
            proxy_enabled: app.settings.read(cx).proxy.enabled,
        }
    }
}
pub(super) struct LoginView {
    controller: WeakEntity<PlayerApp>,
    focus_pending: bool,
    pub inputs: LoginInputs,
    pub step: LoginStep,
    pub qr: Option<(usize, Vec<bool>)>,
    pub request: u64,
    equalizer: Entity<motion::Equalizer>,
    props: LoginProps,
    subscriptions: Vec<Subscription>,
}
impl LoginView {
    pub fn new(
        controller: WeakEntity<PlayerApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let inputs = LoginInputs {
            phone: cx.new(|cx| InputState::new(window, cx).placeholder("+447700900123")),
            code: cx.new(|cx| InputState::new(window, cx).placeholder("Telegram login code")),
            password: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("2FA password")
                    .masked(true)
            }),
        };
        let mut view = Self {
            controller: controller.clone(),
            focus_pending: true,
            inputs,
            step: LoginStep::Qr,
            qr: None,
            request: 0,
            equalizer: cx.new(|_| motion::Equalizer::new(true)),
            props: LoginProps::default(),
            subscriptions: vec![],
        };
        if let Some(controller) = controller.upgrade() {
            view.subscriptions
                .push(cx.observe(&controller, |view, controller, cx| {
                    let props = LoginProps::from_app(controller.read(cx), cx);
                    if view.props != props {
                        view.props = props;
                        cx.notify();
                    }
                }));
        }
        let owner = view.controller.clone();
        view.subscriptions.push(cx.subscribe_in(
            &view.inputs.code,
            window,
            move |view, input, event, window, cx| {
                if matches!(event, InputEvent::Change) && view.step == LoginStep::Code {
                    let value = input.read(cx).value();
                    let complete = value.len() == 5 && value.chars().all(|v| v.is_ascii_digit());
                    let _ = owner.update(cx, |app, cx| {
                        if app.phase == Phase::Login {
                            app.error = None;
                            if app.busy == 0 && complete {
                                let app = cx.weak_entity();
                                let window = window.window_handle();
                                cx.defer(move |cx| {
                                    let _ = window.update(cx, |_, window, cx| {
                                        let _ =
                                            app.update(cx, |app, cx| app.submit_login(window, cx));
                                    });
                                });
                            }
                            cx.notify();
                        }
                    });
                }
            },
        ));
        for input in [&view.inputs.phone, &view.inputs.code, &view.inputs.password] {
            let owner = view.controller.clone();
            view.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |_, _, event, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        let owner = owner.clone();
                        let window = window.window_handle();
                        cx.defer(move |cx| {
                            let _ = window.update(cx, |_, window, cx| {
                                let _ = owner.update(cx, |app, cx| app.submit_login(window, cx));
                            });
                        });
                    }
                },
            ));
        }
        view
    }
    pub fn set_step(&mut self, step: LoginStep, cx: &mut Context<Self>) {
        if self.step == step {
            return;
        }
        self.step = step.clone();
        self.focus_pending = true;
        cx.notify();
    }
    fn action<E: 'static>(
        &self,
        handler: impl Fn(&mut PlayerApp, &E, &mut Window, &mut Context<PlayerApp>) + 'static,
    ) -> impl Fn(&E, &mut Window, &mut App) + 'static {
        let owner = self.controller.clone();
        move |event, window, cx| {
            let _ = owner.update(cx, |app, cx| handler(app, event, window, cx));
        }
    }
}
impl Render for LoginView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_pending) {
            match self.step {
                LoginStep::Phone => self
                    .inputs
                    .phone
                    .update(cx, |input, cx| input.focus(window, cx)),
                LoginStep::Code => self.inputs.code.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                }),
                LoginStep::Password { .. } => self
                    .inputs
                    .password
                    .update(cx, |input, cx| input.focus(window, cx)),
                LoginStep::Qr => {}
            }
        }

        let busy = self.props.busy;
        let step = match self.step.clone() {
            LoginStep::Qr => {
                let qr = self.qr.clone();
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_center()
                            .child(muted("Scan with Telegram on your phone"))
                            .child(
                                muted("SETTINGS → DEVICES → ADD DEVICE")
                                    .font_family(mono_font())
                                    .text_xs()
                                    .line_height(px(16.)),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded(px(16.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(0xffffff))
                            .relative()
                            .child(
                                canvas(
                                    move |bounds, _, _| (bounds, qr.clone()),
                                    |_, (bounds, qr), window, _| {
                                        window.paint_quad(fill(bounds, rgb(0xffffff)));
                                        if let Some((width, modules)) = qr {
                                            let module = bounds.size.width / (width + 8) as f32;
                                            for y in 0..width {
                                                for x in 0..width {
                                                    if modules[y * width + x] {
                                                        window.paint_quad(fill(
                                                            Bounds::new(
                                                                bounds.origin
                                                                    + point(
                                                                        module * (x + 4) as f32,
                                                                        module * (y + 4) as f32,
                                                                    ),
                                                                size(module.ceil(), module.ceil()),
                                                            ),
                                                            rgb(0x000000),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    },
                                )
                                .size(px(224.)),
                            )
                            .when(self.qr.is_none(), |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(rgb(0x666666))
                                        .font_family(mono_font())
                                        .text_xs()
                                        .child(if busy { "Generating…" } else { "Loading…" }),
                                )
                            }),
                    )
                    .when(self.props.error.is_some(), |el| {
                        el.child(
                            button("refresh-qr", "Refresh QR code")
                                .ghost()
                                .disabled(busy)
                                .on_click(self.action(|app, _, _, cx| app.start_qr(true, cx))),
                        )
                    })
                    .child(
                        button("use-phone", "Use phone number instead")
                            .outline()
                            .w_full()
                            .disabled(busy)
                            .on_click(self.action(|app, _, window, cx| {
                                app.sign_in.update(cx, |login, cx| {
                                    login.request += 1;
                                    cx.notify();
                                });
                                app.sign_in
                                    .update(cx, |login, cx| login.set_step(LoginStep::Phone, cx));
                                app.error = None;
                                app.login_inputs(cx)
                                    .phone
                                    .update(cx, |s, cx| s.focus(window, cx));
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            }
            LoginStep::Phone => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    Input::new(&self.inputs.phone)
                        .aria_label("Phone number")
                        .disabled(busy),
                )
                .child(
                    Button::new("send-code")
                        .primary()
                        .label(if busy {
                            "Sending…"
                        } else {
                            "Send login code"
                        })
                        .disabled(busy)
                        .on_click(self.action(|app, _, window, cx| app.submit_login(window, cx))),
                )
                .child(
                    button("use-qr", "Use QR instead")
                        .outline()
                        .disabled(busy)
                        .on_click(self.action(|app, _, _, cx| app.start_qr(false, cx))),
                )
                .into_any_element(),
            LoginStep::Code => {
                let value = self.inputs.code.read(cx).value();
                let focused = self
                    .inputs
                    .code
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window);
                let input = self.inputs.code.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        muted("Check the Telegram app chat with \"Telegram\" for your code.")
                            .text_center(),
                    )
                    .child(
                        div()
                            .id("login-code-cells")
                            .relative()
                            .h(px(36.))
                            .w_full()
                            .flex()
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                input.update(cx, |s, cx| s.focus(window, cx))
                            })
                            .children((0..5).map(|index| {
                                div()
                                    .id(("login-code-cell", index))
                                    .test_support()
                                    .relative()
                                    .flex_1()
                                    .h_full()
                                    .border_1()
                                    .border_color(rgb(if focused && index == value.len().min(4) {
                                        PRIMARY
                                    } else {
                                        BORDER
                                    }))
                                    .when(index > 0, |el| el.border_l_0())
                                    .when(index == 0, |el| el.rounded_l(px(12.)))
                                    .when(index == 4, |el| el.rounded_r(px(12.)))
                                    .bg(rgb(SECONDARY))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_sm()
                                    .child(
                                        value
                                            .chars()
                                            .nth(index)
                                            .map(|v| v.to_string())
                                            .unwrap_or_default(),
                                    )
                            }))
                            // A real native Input owns IME, paste, selection and accessibility.
                            // The five cells above only present its controlled value.
                            .child(
                                Input::new(&self.inputs.code)
                                    .id("login-code")
                                    .aria_label("Login code")
                                    .disabled(busy)
                                    .absolute()
                                    .inset_0()
                                    .opacity(0.),
                            ),
                    )
                    .when(busy, |el| el.child(muted("Verifying…").text_center()))
                    .child(
                        button("change-phone", "Back to phone number")
                            .outline()
                            .disabled(busy)
                            .on_click(self.action(|app, _, window, cx| {
                                app.sign_in
                                    .update(cx, |login, cx| login.set_step(LoginStep::Phone, cx));
                                app.error = None;
                                app.login_inputs(cx)
                                    .phone
                                    .update(cx, |s, cx| s.focus(window, cx));
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            }
            LoginStep::Password { hint, qr } => div()
                .flex()
                .flex_col()
                .gap_3()
                .when_some(hint, |el, hint| {
                    el.child(muted(format!("Password hint: {hint}")).text_center())
                })
                .child(
                    Input::new(&self.inputs.password)
                        .aria_label("Two-step verification password")
                        .disabled(busy),
                )
                .child(
                    Button::new("verify-password")
                        .primary()
                        .label(if busy {
                            "Signing in…"
                        } else if qr {
                            "Continue"
                        } else {
                            "Sign in"
                        })
                        .disabled(busy)
                        .on_click(self.action(|app, _, window, cx| app.submit_login(window, cx))),
                )
                .when(qr, |el| {
                    el.child(
                        button("password-back-qr", "Back to QR login")
                            .outline()
                            .disabled(busy)
                            .on_click(self.action(|app, _, _, cx| app.start_qr(false, cx))),
                    )
                })
                .child(
                    button("password-use-phone", "Back to phone number")
                        .outline()
                        .disabled(busy)
                        .on_click(self.action(|app, _, window, cx| {
                            app.sign_in.update(cx, |login, cx| {
                                login.request += 1;
                                cx.notify();
                            });
                            app.sign_in
                                .update(cx, |login, cx| login.set_step(LoginStep::Phone, cx));
                            app.error = None;
                            app.login_inputs(cx)
                                .phone
                                .update(cx, |s, cx| s.focus(window, cx));
                            cx.notify();
                        })),
                )
                .into_any_element(),
        };
        div().id("login-shell").flex_1().min_h_0().overflow_y_scroll().bg(rgb(BG)).px_5().pt(px(if window.viewport_size().width<px(640.) {64.}else{80.})).pb_12().flex().justify_center()
            .child(div().w(px(416.)).max_w_full().flex().flex_col().items_center()
                .child(div().w(px(32.)).mb_5().child(self.equalizer.clone()))
                .child(div().mb_8().flex().text_size(px(36.)).line_height(px(40.)).font_weight(FontWeight::SEMIBOLD).child(div().text_color(rgb(FG)).child("Sound")).child(div().text_color(rgb(PRIMARY)).child("grammy")))
                .child(div().w(px(300.)).max_w_full().child(step)
                    .when_some(self.props.error.clone(),|el,error|el.child(div().mt_2().text_sm().text_color(rgb(ERROR)).text_center().child(error))))
                .child(button("login-proxy",if self.props.proxy_enabled {"Proxy: On"}else{"Proxy: Off"}).ghost().w(px(300.)).max_w_full().mt_6().text_color(rgb(MUTED)).icon(glyph(IconName::Circle,8.)).on_click(self.action(|app,_,window,cx|app.open_panel(Panel::Proxy,window,cx))))
                .child(muted("SoundGrammy is an independent app that uses the Telegram API. It is not affiliated with or endorsed by Telegram.").w(px(320.)).max_w_full().mt_6().text_xs().line_height(px(19.5)).text_center())).into_any_element()
    }
}
