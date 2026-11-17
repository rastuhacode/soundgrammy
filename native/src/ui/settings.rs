//! Retained settings state, native form inputs and settings workflows.
use super::views::{button, glyph, heading, muted, BORDER, FG, MUTED, PRIMARY};
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::base::Checkbox;
use gpui_kit::component::{
    input::{Input, Textarea},
    Disableable,
};

#[derive(Clone)]
pub(super) struct SettingsInputs {
    pub proxy_link: Entity<TextareaState>,
    pub proxy_server: Entity<InputState>,
    pub proxy_port: Entity<InputState>,
    pub proxy_secret: Entity<InputState>,
    pub cache_limit: Entity<InputState>,
    pub cache_ttl: Entity<InputState>,
}
pub(super) struct SettingsState {
    pub inputs: SettingsInputs,
    pub proxy: crate::proxy_settings::ProxySettings,
    pub proxy_status: Option<crate::proxy_settings::ProxySettingsView>,
    pub cache_settings: Option<crate::cache::CacheSettings>,
    pub cache_usage: Option<crate::cache::CacheUsage>,
    pub lastfm: Option<crate::lastfm::models::LastFmStatus>,
    pub statistics_enabled: bool,
    pub sections: std::collections::HashSet<&'static str>,
}
impl SettingsState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            inputs: SettingsInputs {
                proxy_link: cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .rows(3)
                        .placeholder("tg://proxy?server=127.0.0.1&port=1443&secret=…")
                }),
                proxy_server: cx.new(|cx| InputState::new(window, cx).placeholder("Server")),
                proxy_port: cx.new(|cx| InputState::new(window, cx).placeholder("Port")),
                proxy_secret: cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Secret")
                        .masked(true)
                }),
                cache_limit: cx
                    .new(|cx| InputState::new(window, cx).placeholder("Size limit in GB")),
                cache_ttl: cx.new(|cx| InputState::new(window, cx).placeholder("Keep for days")),
            },
            proxy: Default::default(),
            proxy_status: None,
            cache_settings: None,
            cache_usage: None,
            lastfm: None,
            statistics_enabled: true,
            sections: Default::default(),
        }
    }
}

pub(super) fn check(
    id: &'static str,
    label: &'static str,
    checked: bool,
    toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Checkbox {
    Checkbox::new(id)
        .checked(checked)
        .accessibility_label(label)
        .flex()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .child(check_mark(checked, 16.))
        .child(label)
        .on_change(move |_, event, window, cx| toggle(event, window, cx))
}
pub(super) fn check_mark(checked: bool, size: f32) -> gpui_kit::base::CheckboxIndicator {
    check_indicator(
        if checked {
            gpui_kit::base::CheckboxState::Checked
        } else {
            gpui_kit::base::CheckboxState::Unchecked
        },
        size,
    )
}
pub(super) fn check_indicator(
    state: gpui_kit::base::CheckboxState,
    size: f32,
) -> gpui_kit::base::CheckboxIndicator {
    use gpui_kit::base::CheckboxState::*;
    gpui_kit::base::CheckboxIndicator::new()
        .state(state)
        .size(px(size))
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(rgb(if state == Checked { PRIMARY } else { BORDER }))
        .flex()
        .items_center()
        .justify_center()
        .when(state == Checked, |el| el.bg(rgb(PRIMARY)))
        .when(state == Indeterminate, |el| el.bg(rgb(SECONDARY)))
        .when(state != Unchecked, |el| {
            el.child(
                glyph(IconName::Check, size - 2.).text_color(rgb(if state == Checked {
                    ON_PRIMARY
                } else {
                    MUTED
                })),
            )
        })
}
fn description(text: impl Into<SharedString>) -> Stateful<Div> {
    muted(text).text_xs().line_height(px(16.))
}
fn field(label: &'static str, input: &Entity<InputState>) -> Div {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().font_weight(FontWeight::MEDIUM).child(label))
        .child(Input::new(input).aria_label(label))
}

impl PlayerApp {
    fn settings_section(
        &self,
        title: &'static str,
        content: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.settings.read(cx).sections.contains(title);
        gpui_kit::base::AccordionItem::new()
            .open(open)
            .w_full()
            .flex()
            .flex_col()
            .header(gpui_kit::base::AccordionHeader::new(
                gpui_kit::base::AccordionTrigger::new(title)
                    .aria_label(title)
                    .h(px(40.))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .rounded(px(12.))
                    .hover(|el| el.bg(rgb(SECONDARY)))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(
                        glyph(
                            if open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            },
                            16.,
                        )
                        .text_color(rgb(MUTED)),
                    )
                    .on_change({
                        let change = cx.listener(move |app, _: &ClickEvent, _, cx| {
                            if !app.settings.update(cx, |feature, cx| {
                                let result = feature.sections.remove(title);
                                cx.notify();
                                result
                            }) {
                                app.settings.update(cx, |feature, cx| {
                                    let result = feature.sections.insert(title);
                                    cx.notify();
                                    result
                                });
                            }
                            cx.notify();
                        });
                        move |_, event, window, cx| change(event, window, cx)
                    }),
            ))
            .panel(
                gpui_kit::base::AccordionPanel::new()
                    .child(div().pl_7().pr_5().pt_3().pb_4().child(content)),
            )
            .into_any_element()
    }
    pub(super) fn proxy_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div().flex().flex_col().gap_4()
            .child(description("Route Telegram through a local helper such as tg-ws-proxy. This could be helpful to avoid censorship."))
            .child(check("proxy-enabled","Use MTProto proxy",self.settings.read(cx).proxy.enabled,cx.listener(|app,_,_,cx|{app.settings.update(cx, |feature, cx| { feature.proxy.enabled= !feature.proxy.enabled; cx.notify(); });cx.notify();})))
            .child(div().flex().flex_col().gap_2().child(div().font_weight(FontWeight::MEDIUM).child("Paste link")).child(Textarea::new(&self.settings_inputs(cx).proxy_link).aria_label("Paste link").h(px(76.)).font_family(mono_font()).text_xs()))
            .child(button("parse-proxy","Parse link").w_full().outline().disabled(self.settings_inputs(cx).proxy_link.read(cx).value().is_empty())
                .on_click(cx.listener(|app,_,window,cx|app.parse_proxy(window,cx))))
            .child(div().flex().gap_3().child(field("Server",&self.settings_inputs(cx).proxy_server).flex_1()).child(field("Port",&self.settings_inputs(cx).proxy_port).w(px(100.))))
            .child(field("Secret",&self.settings_inputs(cx).proxy_secret))
            .when_some(self.settings.read(cx).proxy_status.as_ref().and_then(|s|s.apply_error.as_ref()),|el,error|el.child(description(error.to_string()).text_color(rgb(ERROR))))
            .child(button("apply-proxy","Apply & reconnect").disabled(self.busy>0).on_click(cx.listener(|app,_,_,cx|app.save_proxy(cx))))
            .into_any_element()
    }
    fn lastfm_view(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::lastfm::models::LastFmAuthState as State;
        div().flex().flex_col().gap_4()
            .child(div().flex().items_center().gap_2().child(heading("LAST.FM").text_color(rgb(MUTED)))
                .child(div().px_2().rounded_full().border_1().border_color(rgb(BORDER)).text_xs().child("EXPEREMENTAL")))
            .child(description("Send the music you listen to in SoundGrammy to your Last.fm profile. Tracks are added after half the track or four minutes, whichever comes first."))
            .when(self.settings.read(cx).lastfm.is_none(),|el|el.child(description("Loading…")))
            .when_some(self.settings.read(cx).lastfm.as_ref(),|el,s| {
                let connected=s.state==State::Connected;
                el.when(s.state==State::UnavailableInBuild,|el|el.child(description("Last.fm is unavailable in this build.")))
                    .when(!connected && s.state!=State::WaitingForBrowserApproval,|el|el.child(button("lastfm-connect","Connect Last.fm").self_start().disabled(s.state==State::UnavailableInBuild)
                        .on_click(cx.listener(|app,_,_,cx|app.lastfm_action("connect",cx)))))
                    .when(s.state==State::WaitingForBrowserApproval,|el|el.child(description("Waiting for authorization…"))
                        .child(div().flex().gap_2().flex_wrap()
                            .child(button("lastfm-complete","I've authorized SoundGrammy").on_click(cx.listener(|app,_,_,cx|app.lastfm_action("complete",cx))))
                            .child(button("lastfm-cancel","Cancel").outline().on_click(cx.listener(|app,_,_,cx|app.lastfm_action("cancel",cx))))))
                    .when(connected,|el|el
                        .child(div().flex().items_center().gap_2().child(div().size(px(8.)).rounded_full().bg(rgb(0x00bc8c)))
                            .child(format!("Connected as @{}",s.username.as_deref().unwrap_or_default())))
                        .child(check("lastfm-enabled","Scrobble SoundGrammy listening",s.enabled,cx.listener(|app,_,_,cx|app.lastfm_action("toggle",cx))))
                        .when(!s.enabled,|el|el.child(description("Pending uploads are retained and will resume when scrobbling is enabled.")))
                        .child(div().flex().gap_3().text_xs().line_height(px(16.))
                            .child(div().flex().flex_col().gap_1().text_color(rgb(MUTED)).child("Last scrobble").child("Waiting to upload"))
                            .child(div().flex().flex_col().gap_1().child(s.last_scrobble_at_ms.map(|time| {
                                let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
                                let minutes=(now-time).max(0)/60000;
                                if minutes<1 {"Just now".into()} else if minutes<60 {format!("{minutes}m ago")} else {format!("{}h ago",minutes/60)}
                            }).unwrap_or_else(||"Never".into())).child(s.pending_count.to_string())))
                        .child(div().flex().gap_2().flex_wrap()
                            .child(button("lastfm-profile","Open Last.fm profile").outline().on_click(cx.listener(|app,_,_,cx|app.lastfm_action("profile",cx))))
                            .when(s.pending_count>0,|el|el.child(button("lastfm-flush","Retry pending scrobbles").outline().on_click(cx.listener(|app,_,_,cx|app.lastfm_action("flush",cx)))))
                            .child(button("lastfm-disconnect","Disconnect").outline().on_click(cx.listener(|app,_,_,cx|{app.show_panel(Panel::Confirm(Confirmation::DisconnectLastFm));cx.notify();})))))
                    .when_some(s.last_error.as_ref(),|el,e|el.child(description(e.message.clone()).text_color(rgb(ERROR))))
                    .when_some(s.last_metadata_warning.as_ref(),|el,e|el.child(description(e.message.clone())))
                    .children(s.retained_queues.iter().map(|q|description(format!("Retained for @{}: {} scrobbles. These will upload only after reconnecting that account.",q.username,q.count))))
            }).into_any_element()
    }
    pub(super) fn settings_view(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.settings.read(cx).statistics_enabled;
        gpui_kit::base::Accordion::new("settings-sections").flex().flex_col().gap_2()
            .child(self.settings_section("Audio cache",div().flex().flex_col().gap_4()
                .child(description("Cached audio stays in the app and accessible offline, but takes up physical space on your device."))
                .when_some(self.settings.read(cx).cache_usage.as_ref(),|el,u| {
                    let used=u.used_bytes as f64/1024_f64.powi(3);
                    let limit=self.settings.read(cx).cache_settings.as_ref().map(|s|s.limit_bytes).unwrap_or(0) as f64/1024_f64.powi(3);
                    let fraction=if limit>0. {(used/limit).clamp(0.,1.)} else {0.};
                    el.child(div().p_3().border_1().border_color(rgb(BORDER)).rounded(px(16.)).flex().flex_col().gap_2()
                        .child(div().flex().items_center().justify_between().child(muted("Usage"))
                            .child(div().font_family(mono_font()).text_xs().line_height(px(16.)).child(format!("{used:.2} GB / {limit:.2} GB ({}%)",(fraction*100.).round()))))
                        .child(div().h(px(6.)).w_full().rounded_full().bg(rgb(SECONDARY)).child(div().h_full().w(relative(fraction as f32)).rounded_full().bg(rgb(PRIMARY))))
                        .child(description(format!("{} FILES",u.file_count)).font_family(mono_font())))
                })
                .child(div().flex().gap_3().child(field("Size limit (GB)",&self.settings_inputs(cx).cache_limit).flex_1()).child(field("Keep for (days)",&self.settings_inputs(cx).cache_ttl).flex_1()))
                .child(div().flex().gap_2()
                    .child(button("save-cache","Save cache settings").on_click(cx.listener(|app,_,_,cx|app.save_cache_settings(cx))))
                    .child(button("clear-cache","Clear cache").outline().on_click(cx.listener(|app,_,_,cx|{app.show_panel(Panel::Confirm(Confirmation::ClearCache));cx.notify();})))) ,cx))
            .child(self.settings_section("Listening statistics",div().flex().flex_col().gap_4()
                .child(description("Track your listening activity to build the Popular and Recent playlists. Turning this off keeps existing history but hides those playlists."))
                .child(check("toggle-statistics","Collect listening statistics",enabled,cx.listener(move |app,_,_,cx| {
                    app.mutation(cx,move |host|async move{commands::listen_stats::set_listen_statistics_enabled(&host.state::<AppState>(),host,!enabled).await},move |app,_,cx|app.set_statistics_enabled(!enabled,cx));
                })))
                .child(button("clear-statistics","Clear statistics").self_start().outline().on_click(cx.listener(|app,_,_,cx|{app.show_panel(Panel::Confirm(Confirmation::ClearStatistics));cx.notify();}))),cx))
            .child(self.settings_section("Integrations",div().flex().flex_col().gap_4()
                .child(self.lastfm_view(cx))
                .child(div().border_t_1().border_color(rgb(BORDER)).pt_4().child(heading("MTPROTO PROXY").text_color(rgb(MUTED))))
                .child(self.proxy_view(cx)),cx))
            .child(self.settings_section("Diagnostics",div().flex().flex_col().gap_4()
                .child(description("Keep a local history of application errors for troubleshooting. Logs may contain track identifiers and local file paths."))
                .child(check("logs-enabled","Enable diagnostic logs",self.preferences.logs_enabled,cx.listener(|app,_,_,cx|{app.preferences.logs_enabled= !app.preferences.logs_enabled;app.save_preferences();cx.notify();})))
                .child(button("open-logs",format!("View logs ({})",self.logs.entries().len())).self_start().outline().on_click(cx.listener(|app,_,window,cx|app.open_panel(Panel::Logs,window,cx)))),cx))
            .child(self.settings_section("Fullscreen player",div().flex().flex_col().gap_4()
                .child(description("Prevent normal inactivity from dimming or turning off the display while the fullscreen player is open. This may increase battery usage."))
                .child(check("keep-awake","Keep display awake",self.preferences.keep_awake,cx.listener(|app,_,_,cx|{app.preferences.keep_awake= !app.preferences.keep_awake;app.save_preferences();app.apply_display_awake(cx);cx.notify();})))
                .child(div().border_t_1().border_color(rgb(BORDER)).pt_0().child(heading("ARTWORK BOUNCE").text_color(rgb(MUTED))))
                .child(description("Move fullscreen artwork with bass hits and let the palette follow bass, mid and treble dynamics."))
                .child(check("bounce-enabled","Enable artwork bounce",self.preferences.bounce.enabled,cx.listener(|app,_,_,cx|{app.preferences.bounce.enabled= !app.preferences.bounce.enabled;app.save_preferences();app.request_effects(cx);cx.notify();})))
                .child(button("reset-bounce","Reset defaults").self_start().outline().on_click(cx.listener(|app,_,_,cx|{app.preferences.bounce=Default::default();app.save_preferences();app.request_effects(cx);cx.notify();}))),cx))
            .text_color(rgb(FG)).into_any_element()
    }
}

impl PlayerApp {
    pub(super) fn load_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        let host = self.host();
        let proxy = crate::proxy_settings::load(&host.state::<AppState>().db).unwrap_or_default();
        self.settings.update(cx, |feature, cx| {
            feature.proxy = proxy.clone();
            cx.notify();
        });
        self.settings_inputs(cx).proxy_link.update(cx, |s, cx| {
            s.set_value(
                if proxy.is_configured() {
                    proxy.to_proxy_link()
                } else {
                    String::new()
                },
                window,
                cx,
            )
        });
        self.settings_inputs(cx)
            .proxy_server
            .update(cx, |s, cx| s.set_value(proxy.server, window, cx));
        self.settings_inputs(cx)
            .proxy_port
            .update(cx, |s, cx| s.set_value(proxy.port.to_string(), window, cx));
        self.settings_inputs(cx)
            .proxy_secret
            .update(cx, |s, cx| s.set_value(proxy.secret, window, cx));
        if let Ok(settings) = crate::cache::get_cache_settings(&host.state::<AppState>()) {
            self.settings_inputs(cx).cache_limit.update(cx, |s, cx| {
                s.set_value(
                    format!("{}", settings.limit_bytes as f64 / 1024_f64.powi(3)),
                    window,
                    cx,
                )
            });
            self.settings_inputs(cx).cache_ttl.update(cx, |s, cx| {
                s.set_value(format!("{}", settings.ttl_secs / 86400), window, cx)
            });
            self.settings.update(cx, |feature, cx| {
                feature.cache_settings = Some(settings);
                cx.notify();
            });
        }
        self.task(
            cx,
            async move {
                let state = host.state::<AppState>();
                let usage = commands::get_cache_usage(&state).await?;
                let enabled = commands::listen_stats::get_listen_statistics_enabled(&state).await?;
                let lastfm = commands::lastfm::get_lastfm_status(&state).await?;
                let proxy = commands::get_proxy_settings(&state).await?;
                Ok::<_, crate::error::AppError>((usage, enabled, lastfm, proxy))
            },
            |app, (usage, enabled, lastfm, proxy), cx| {
                app.settings.update(cx, |feature, cx| {
                    feature.proxy_status = Some(proxy);
                    cx.notify();
                });
                app.settings.update(cx, |feature, cx| {
                    feature.cache_usage = Some(usage);
                    cx.notify();
                });
                app.set_statistics_enabled(enabled, cx);
                app.settings.update(cx, |feature, cx| {
                    feature.lastfm = Some(lastfm);
                    cx.notify();
                });
            },
        );
    }

    pub(super) fn save_cache_settings(&mut self, cx: &mut Context<Self>) {
        let limit = finite_nonnegative(
            &self.settings_inputs(cx).cache_limit.read(cx).value(),
            "Size limit",
        );
        let ttl = finite_nonnegative(
            &self.settings_inputs(cx).cache_ttl.read(cx).value(),
            "Keep-for days",
        );
        let (limit, ttl) = match (limit, ttl) {
            (Ok(l), Ok(t)) => (l, t),
            (Err(e), _) | (_, Err(e)) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        self.mutation(
            cx,
            move |host| async move {
                let state = host.state::<AppState>();
                let settings = commands::set_cache_settings(
                    &state,
                    host,
                    Some((limit * 1024_f64.powi(3)).round() as i64),
                    Some((ttl * 86400.).round() as i64),
                )
                .await?;
                let usage = commands::get_cache_usage(&state).await?;
                Ok((settings, usage))
            },
            |app, (settings, usage), cx| {
                app.settings.update(cx, |feature, cx| {
                    feature.cache_settings = Some(settings);
                    cx.notify();
                });
                app.settings.update(cx, |feature, cx| {
                    feature.cache_usage = Some(usage);
                    cx.notify();
                });
                app.notice = Some("Cache settings saved".into());
            },
        );
    }

    pub(super) fn save_proxy(&mut self, cx: &mut Context<Self>) {
        let settings = crate::proxy_settings::ProxySettings {
            enabled: self.settings.read(cx).proxy.enabled,
            server: self
                .settings_inputs(cx)
                .proxy_server
                .read(cx)
                .value()
                .to_string(),
            port: match self
                .settings_inputs(cx)
                .proxy_port
                .read(cx)
                .value()
                .parse::<u16>()
            {
                Ok(p) => p,
                Err(_) if !self.settings.read(cx).proxy.enabled => 0,
                Err(_) => {
                    self.error = Some("Enter a port between 1 and 65535".into());
                    return;
                }
            },
            secret: self
                .settings_inputs(cx)
                .proxy_secret
                .read(cx)
                .value()
                .to_string(),
        };
        if let Err(e) = settings.validate() {
            self.error = Some(e.to_string());
            return;
        }
        self.mutation(
            cx,
            move |host| async move {
                commands::set_proxy_settings(
                    &host.state::<AppState>(),
                    settings.enabled,
                    settings.server,
                    settings.port,
                    settings.secret,
                )
                .await
            },
            |app, _, cx| {
                app.panel = app.panel_stack.pop();
                app.online = false;
                if app.phase == Phase::Ready {
                    app.sync(false, cx);
                }
            },
        );
    }

    pub(super) fn parse_proxy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match crate::proxy_settings::from_proxy_link(
            &self.settings_inputs(cx).proxy_link.read(cx).value(),
        ) {
            Ok(settings) => {
                self.settings_inputs(cx)
                    .proxy_server
                    .update(cx, |s, cx| s.set_value(settings.server.clone(), window, cx));
                self.settings_inputs(cx).proxy_port.update(cx, |s, cx| {
                    s.set_value(settings.port.to_string(), window, cx)
                });
                self.settings_inputs(cx)
                    .proxy_secret
                    .update(cx, |s, cx| s.set_value(settings.secret.clone(), window, cx));
                self.settings.update(cx, |feature, cx| {
                    feature.proxy = settings;
                    cx.notify();
                });
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn lastfm_action(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let enabled = !self
            .settings
            .read(cx)
            .lastfm
            .as_ref()
            .is_some_and(|s| s.enabled);
        self.mutation(
            cx,
            move |host| async move {
                let state = host.state::<AppState>();
                match action {
                    "connect" => commands::lastfm::start_lastfm_auth(&state, host).await,
                    "complete" => commands::lastfm::complete_lastfm_auth(&state, host).await,
                    "cancel" => commands::lastfm::cancel_lastfm_auth(&state, host).await,
                    "disconnect" => {
                        commands::lastfm::disconnect_lastfm(&state, host, Some("retain".into()))
                            .await
                    }
                    "disconnect-delete" => {
                        commands::lastfm::disconnect_lastfm(&state, host, Some("delete".into()))
                            .await
                    }
                    "toggle" => commands::lastfm::set_lastfm_enabled(&state, host, enabled).await,
                    "profile" => {
                        commands::lastfm::open_lastfm_profile(&state, host).await?;
                        commands::lastfm::get_lastfm_status(&state).await
                    }
                    _ => {
                        commands::lastfm::flush_lastfm_queue(&state).await?;
                        commands::lastfm::get_lastfm_status(&state).await
                    }
                }
            },
            |app, status, cx| {
                app.settings.update(cx, |feature, cx| {
                    feature.lastfm = Some(status);
                    cx.notify();
                })
            },
        );
    }
}
