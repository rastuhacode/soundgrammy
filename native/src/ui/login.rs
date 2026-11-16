//! The same narrow sign-in shell and step layouts as the original client.
use super::views::{button, glyph, muted, BG, BORDER, FG, MUTED, PRIMARY};
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    Disableable,
};
impl PlayerApp {
    pub(super) fn login_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let focus_step = match self.login {
            LoginStep::Qr => "qr",
            LoginStep::Phone => "phone",
            LoginStep::Code => "code",
            LoginStep::Password { .. } => "password",
        };
        if self.login_focus_step != Some(focus_step) {
            self.login_focus_step = Some(focus_step);
            match focus_step {
                "phone" => self.inputs.phone.update(cx, |s, cx| s.focus(window, cx)),
                "code" => self.inputs.code.update(cx, |s, cx| {
                    s.set_value("", window, cx);
                    s.focus(window, cx);
                }),
                "password" => self.inputs.password.update(cx, |s, cx| s.focus(window, cx)),
                _ => {}
            }
        }
        let busy = self.busy > 0;
        let step = match self.login.clone() {
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
                    .when(self.error.is_some(), |el| {
                        el.child(
                            button("refresh-qr", "Refresh QR code")
                                .ghost()
                                .disabled(busy)
                                .on_click(cx.listener(|app, _, _, cx| app.start_qr(true, cx))),
                        )
                    })
                    .child(
                        button("use-phone", "Use phone number instead")
                            .outline()
                            .w_full()
                            .disabled(busy)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.qr_request += 1;
                                app.login = LoginStep::Phone;
                                app.error = None;
                                app.inputs.phone.update(cx, |s, cx| s.focus(window, cx));
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
                        .on_click(cx.listener(|app, _, window, cx| app.submit_login(window, cx))),
                )
                .child(
                    button("use-qr", "Use QR instead")
                        .outline()
                        .disabled(busy)
                        .on_click(cx.listener(|app, _, _, cx| app.start_qr(false, cx))),
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
                                    .bg(rgb(0x272727))
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
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.login = LoginStep::Phone;
                                app.error = None;
                                app.inputs.phone.update(cx, |s, cx| s.focus(window, cx));
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
                        .on_click(cx.listener(|app, _, window, cx| app.submit_login(window, cx))),
                )
                .when(qr, |el| {
                    el.child(
                        button("password-back-qr", "Back to QR login")
                            .outline()
                            .disabled(busy)
                            .on_click(cx.listener(|app, _, _, cx| app.start_qr(false, cx))),
                    )
                })
                .child(
                    button("password-use-phone", "Back to phone number")
                        .outline()
                        .disabled(busy)
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.qr_request += 1;
                            app.login = LoginStep::Phone;
                            app.error = None;
                            app.inputs.phone.update(cx, |s, cx| s.focus(window, cx));
                            cx.notify();
                        })),
                )
                .into_any_element(),
        };
        div().id("login-shell").flex_1().min_h_0().overflow_y_scroll().bg(rgb(BG)).px_5().pt(px(if window.viewport_size().width<px(640.) {64.}else{80.})).pb_12().flex().justify_center()
            .child(div().w(px(416.)).max_w_full().flex().flex_col().items_center()
                .child(div().w(px(32.)).mb_5().child(self.login_equalizer.clone()))
                .child(div().mb_8().flex().text_size(px(36.)).line_height(px(40.)).font_weight(FontWeight::SEMIBOLD).child(div().text_color(rgb(FG)).child("Sound")).child(div().text_color(rgb(PRIMARY)).child("grammy")))
                .child(div().w(px(300.)).max_w_full().child(step)
                    .when_some(self.error.clone(),|el,error|el.child(div().mt_2().text_sm().text_color(rgb(0xff656a)).text_center().child(error))))
                .child(button("login-proxy",if self.proxy.enabled {"Proxy: On"}else{"Proxy: Off"}).ghost().w(px(300.)).max_w_full().mt_6().text_color(rgb(MUTED)).icon(glyph(IconName::Circle,8.)).on_click(cx.listener(|app,_,window,cx|app.open_panel(Panel::Proxy,window,cx))))
                .child(muted("SoundGrammy is an independent app that uses the Telegram API. It is not affiliated with or endorsed by Telegram.").w(px(320.)).max_w_full().mt_6().text_xs().line_height(px(19.5)).text_center())).into_any_element()
    }
}
