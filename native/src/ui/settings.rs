//! Settings retain the previous application's disclosure and form layouts.
use super::views::{button, glyph, heading, muted, BORDER, FG, MUTED, PRIMARY};
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::base::Checkbox;
use gpui_kit::component::{
    input::{Input, Textarea},
    Disableable,
};

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
        .when(state == Indeterminate, |el| el.bg(rgb(0x272727)))
        .when(state != Unchecked, |el| {
            el.child(
                glyph(IconName::Check, size - 2.).text_color(rgb(if state == Checked {
                    0x111111
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
        let open = self.settings_sections.contains(title);
        gpui_kit::base::Collapsible::new()
            .open(open)
            .w_full()
            .flex()
            .flex_col()
            .child(
                gpui_kit::base::Button::new(title)
                    .accessibility_label(title)
                    .aria_expanded(open)
                    .h(px(40.))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .rounded(px(12.))
                    .hover(|el| el.bg(rgb(0x272727)))
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
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if !app.settings_sections.remove(title) {
                            app.settings_sections.insert(title);
                        }
                        cx.notify();
                    })),
            )
            .content(div().pl_7().pr_5().pt_3().pb_4().child(content))
            .into_any_element()
    }
    pub(super) fn proxy_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div().flex().flex_col().gap_4()
            .child(description("Route Telegram through a local helper such as tg-ws-proxy. This could be helpful to avoid censorship."))
            .child(check("proxy-enabled","Use MTProto proxy",self.proxy.enabled,cx.listener(|app,_,_,cx|{app.proxy.enabled= !app.proxy.enabled;cx.notify();})))
            .child(div().flex().flex_col().gap_2().child(div().font_weight(FontWeight::MEDIUM).child("Paste link")).child(Textarea::new(&self.inputs.proxy_link).aria_label("Paste link").h(px(76.)).font_family(mono_font()).text_xs()))
            .child(button("parse-proxy","Parse link").w_full().outline().disabled(self.inputs.proxy_link.read(cx).value().is_empty())
                .on_click(cx.listener(|app,_,window,cx|app.parse_proxy(window,cx))))
            .child(div().flex().gap_3().child(field("Server",&self.inputs.proxy_server).flex_1()).child(field("Port",&self.inputs.proxy_port).w(px(100.))))
            .child(field("Secret",&self.inputs.proxy_secret))
            .when_some(self.proxy_status.as_ref().and_then(|s|s.apply_error.as_ref()),|el,error|el.child(description(error.to_string()).text_color(rgb(0xff656a))))
            .child(button("apply-proxy","Apply & reconnect").disabled(self.busy>0).on_click(cx.listener(|app,_,_,cx|app.save_proxy(cx))))
            .into_any_element()
    }
    fn lastfm_view(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::lastfm::models::LastFmAuthState as State;
        div().flex().flex_col().gap_4()
            .child(div().flex().items_center().gap_2().child(heading("LAST.FM").text_color(rgb(MUTED)))
                .child(div().px_2().rounded_full().border_1().border_color(rgb(BORDER)).text_xs().child("EXPEREMENTAL")))
            .child(description("Send the music you listen to in SoundGrammy to your Last.fm profile. Tracks are added after half the track or four minutes, whichever comes first."))
            .when(self.lastfm.is_none(),|el|el.child(description("Loading…")))
            .when_some(self.lastfm.as_ref(),|el,s| {
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
                    .when_some(s.last_error.as_ref(),|el,e|el.child(description(e.message.clone()).text_color(rgb(0xff656a))))
                    .when_some(s.last_metadata_warning.as_ref(),|el,e|el.child(description(e.message.clone())))
                    .children(s.retained_queues.iter().map(|q|description(format!("Retained for @{}: {} scrobbles. These will upload only after reconnecting that account.",q.username,q.count))))
            }).into_any_element()
    }
    pub(super) fn settings_view(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.statistics_enabled;
        div().flex().flex_col().gap_2()
            .child(self.settings_section("Audio cache",div().flex().flex_col().gap_4()
                .child(description("Cached audio stays in the app and accessible offline, but takes up physical space on your device."))
                .when_some(self.cache_usage.as_ref(),|el,u| {
                    let used=u.used_bytes as f64/1024_f64.powi(3);
                    let limit=self.cache_settings.as_ref().map(|s|s.limit_bytes).unwrap_or(0) as f64/1024_f64.powi(3);
                    let fraction=if limit>0. {(used/limit).clamp(0.,1.)} else {0.};
                    el.child(div().p_3().border_1().border_color(rgb(BORDER)).rounded(px(16.)).flex().flex_col().gap_2()
                        .child(div().flex().items_center().justify_between().child(muted("Usage"))
                            .child(div().font_family(mono_font()).text_xs().line_height(px(16.)).child(format!("{used:.2} GB / {limit:.2} GB ({}%)",(fraction*100.).round()))))
                        .child(div().h(px(6.)).w_full().rounded_full().bg(rgb(0x272727)).child(div().h_full().w(relative(fraction as f32)).rounded_full().bg(rgb(PRIMARY))))
                        .child(description(format!("{} FILES",u.file_count)).font_family(mono_font())))
                })
                .child(div().flex().gap_3().child(field("Size limit (GB)",&self.inputs.cache_limit).flex_1()).child(field("Keep for (days)",&self.inputs.cache_ttl).flex_1()))
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
                .child(check("bounce-enabled","Enable artwork bounce",self.preferences.bounce.enabled,cx.listener(|app,_,_,cx|{app.preferences.bounce.enabled= !app.preferences.bounce.enabled;app.save_preferences();cx.notify();})))
                .child(button("reset-bounce","Reset defaults").self_start().outline().on_click(cx.listener(|app,_,_,cx|{app.preferences.bounce=Default::default();app.save_preferences();cx.notify();}))),cx))
            .text_color(rgb(FG)).into_any_element()
    }
}
