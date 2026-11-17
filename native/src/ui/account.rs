use super::*;

impl PlayerApp {
    pub(super) fn bootstrap(&mut self, cx: &mut Context<Self>) {
        let host = self.host();
        self.task(
            cx,
            async move { commands::auth::auth_status(&host.state::<AppState>()).await },
            |app, status, cx| {
                if let Some(user) = status.user.filter(|_| status.authorized) {
                    app.authenticated(user, cx);
                } else {
                    app.phase = Phase::Login;
                    app.start_qr(false, cx);
                }
            },
        );
    }

    pub(super) fn authenticated(&mut self, user: AuthUser, cx: &mut Context<Self>) {
        self.user = Some(user);
        self.phase = Phase::Loading;
        self.sign_in.update(cx, |login, cx| {
            login.qr = None;
            cx.notify();
        });
        self.sign_in.update(cx, |login, cx| {
            login.request += 1;
            cx.notify();
        });
        self.hydrate(cx, true);
        let host = self.host();
        self.quiet_task(
            cx,
            async move { commands::get_user_avatar(&host.state::<AppState>()).await },
            |app, path, _| app.avatar = path.map(Into::into),
        );
    }

    pub(super) fn reconnect(&mut self, cx: &mut Context<Self>) {
        if self.reconnect_running {
            return;
        }
        self.reconnect_running = true;
        let host = self.host();
        let epoch = self.epoch;
        cx.spawn(async move |view, cx| {
            let mut delay = 1_u64;
            loop {
                let h = host.clone();
                let result = crate::runtime::spawn(async move {
                    let state = h.state::<AppState>();
                    commands::auth::refresh_auth(&state, h).await
                })
                .await;
                let connected = matches!(&result,Ok(Ok(status)) if status.authorized);
                let current = view
                    .update(cx, |app, cx| {
                        if app.epoch != epoch {
                            return false;
                        }
                        let was_online = app.online;
                        app.online = connected;
                        if connected && !was_online {
                            app.thumbs.retain(|_, path| path.is_some());
                            app.hq_thumbs.clear();
                            let avatar_host = app.host();
                            app.quiet_task(
                                cx,
                                async move {
                                    commands::get_user_avatar(&avatar_host.state::<AppState>())
                                        .await
                                },
                                |app, path, _| app.avatar = path.map(Into::into),
                            );
                            app.sync(false, cx);
                        }
                        if let Ok(Ok(status)) = result {
                            if !status.authorized {
                                app.reset(cx);
                                cx.notify();
                                return false;
                            }
                            if let Some(user) = status.user {
                                app.user = Some(user);
                            }
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !current {
                    break;
                }
                delay = if connected { 30 } else { (delay * 2).min(30) };
                crate::runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(delay)).await
                })
                .await
                .ok();
            }
        })
        .detach();
    }

    pub(super) fn start_qr(&mut self, restart: bool, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        self.sign_in
            .update(cx, |login, cx| login.set_step(LoginStep::Qr, cx));
        self.sign_in.update(cx, |login, cx| {
            login.qr = None;
            cx.notify();
        });
        self.sign_in.update(cx, |login, cx| {
            login.request += 1;
            cx.notify();
        });
        self.poll_qr(restart, true, self.sign_in.read(cx).request, cx);
    }

    pub(super) fn poll_qr(
        &mut self,
        restart: bool,
        initial: bool,
        request: u64,
        cx: &mut Context<Self>,
    ) {
        let host = self.host();
        self.task(
            cx,
            async move {
                let state = host.state::<AppState>();
                if restart {
                    commands::auth::qr_restart(&state).await
                } else if initial {
                    commands::auth::qr_start(&state).await
                } else {
                    commands::auth::qr_poll(&state).await
                }
            },
            move |app, outcome, cx| {
                if app.sign_in.read(cx).request != request
                    || app.sign_in.read(cx).step != LoginStep::Qr
                {
                    return;
                }
                match outcome {
                    QrOutcome::Authorized { user } => app.authenticated(user, cx),
                    QrOutcome::PasswordRequired { hint } => {
                        app.sign_in.update(cx, |login, cx| {
                            login.set_step(LoginStep::Password { qr: true, hint }, cx)
                        });
                        app.sign_in.update(cx, |login, cx| {
                            login.qr = None;
                            cx.notify();
                        });
                    }
                    QrOutcome::Waiting { url, .. } => {
                        if let Ok(code) = qrcode::QrCode::new(url.as_bytes()) {
                            app.sign_in.update(cx, |login, cx| {
                                login.qr = Some((
                                    code.width(),
                                    code.to_colors()
                                        .into_iter()
                                        .map(|v| v == qrcode::Color::Dark)
                                        .collect(),
                                ));
                                cx.notify();
                            });
                        }
                        cx.spawn(async move |view, cx| {
                            crate::runtime::spawn(async {
                                tokio::time::sleep(Duration::from_secs(2)).await
                            })
                            .await
                            .ok();
                            let _ = view.update(cx, |app, cx| {
                                if app.sign_in.read(cx).request == request
                                    && app.sign_in.read(cx).step == LoginStep::Qr
                                {
                                    app.poll_qr(false, false, request, cx);
                                }
                            });
                        })
                        .detach();
                    }
                }
            },
        );
    }

    pub(super) fn submit_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy > 0 || self.host.is_none() {
            return;
        }
        let host = self.host();
        match self.sign_in.read(cx).step.clone() {
            LoginStep::Phone => {
                let phone = match login_phone(self.login_inputs(cx).phone.read(cx).value().as_ref())
                {
                    Ok(value) => value,
                    Err(error) => {
                        self.error = Some(error.to_string());
                        return;
                    }
                };
                self.task(
                    cx,
                    async move {
                        commands::auth::phone_send_code(&host.state::<AppState>(), phone).await
                    },
                    |app, result, cx| match result {
                        PhoneSendCodeOutcome::CodeSent => app
                            .sign_in
                            .update(cx, |login, cx| login.set_step(LoginStep::Code, cx)),
                        PhoneSendCodeOutcome::Authorized { user } => app.authenticated(user, cx),
                    },
                );
            }
            LoginStep::Code => {
                let code = match login_code(self.login_inputs(cx).code.read(cx).value().as_ref()) {
                    Ok(value) => value,
                    Err(error) => {
                        self.error = Some(error.to_string());
                        return;
                    }
                };
                self.task(cx,async move{commands::auth::phone_sign_in(&host.state::<AppState>(),code).await},|app,result,cx|match result{AuthOutcome::Authorized{user}=>app.authenticated(user,cx),AuthOutcome::PasswordRequired{hint}=>app.sign_in.update(cx, |login, cx| login.set_step(LoginStep::Password{qr:false,hint}, cx))});
            }
            LoginStep::Password { qr, .. } => {
                let password = self.login_inputs(cx).password.read(cx).value().to_string();
                self.login_inputs(cx)
                    .password
                    .update(cx, |state, cx| state.set_value("", window, cx));
                if password.is_empty() {
                    self.error = Some("Enter your 2FA password".into());
                    return;
                }
                self.task(
                    cx,
                    async move {
                        let state = host.state::<AppState>();
                        if qr {
                            commands::auth::qr_check_password(&state, password).await
                        } else {
                            commands::auth::phone_check_password(&state, password).await
                        }
                    },
                    |app, user, cx| app.authenticated(user, cx),
                );
            }
            LoginStep::Qr => {}
        }
    }

    pub(super) fn reset(&mut self, cx: &mut Context<Self>) {
        self.dismiss_hover_titles();
        self.reset_inputs = true;
        self.epoch += 1;
        self.account_active
            .store(false, std::sync::atomic::Ordering::Release);
        self.account_active = Arc::new(std::sync::atomic::AtomicBool::new(true));
        self.sign_in.update(cx, |login, cx| {
            login.request += 1;
            cx.notify();
        });
        self.user = None;
        self.phase = Phase::Login;
        self.library.clone().update(cx, |library, cx| {
            *library = Default::default();
            cx.notify();
        });
        self.playback.update(cx, |state, cx| {
            *state = Default::default();
            cx.notify();
        });
        self.thumbs.clear();
        self.download_progress.clear();
        self.hq_thumbs.clear();
        self.thumb_loading.clear();
        self.pending_fullscreen = false;
        self.metadata = None;
        self.avatar = None;
        self.panel = None;
        self.panel_stack.clear();
        self.retained_modal = None;
        self.reset_volume_hover();
        self.busy = 0;
        self.error = None;
        self.notice = None;
        self.jobs.clear();
        self.loading_started = None;
        self.loading_indicator_task = None;
        self.download_results.clear();
        self.download_sections.clear();
        self.settings.update(cx, |feature, cx| {
            feature.lastfm = None;
            cx.notify();
        });
        self.online = false;
        self.syncing = false;
        self.pending_full_sync = false;
        self.reconnect_running = false;
        self.compact_playlist = false;
        self.selection_mode = false;
        self.sign_in.update(cx, |login, cx| {
            login.qr = None;
            cx.notify();
        });
        self.sign_in
            .update(cx, |login, cx| login.set_step(LoginStep::Phone, cx));
        self.bounce_track = None;
        self.bounce_profile = None;
        self.mutations = Default::default();
        self.command_port = None;
        self.request_effects(cx);
    }
}
