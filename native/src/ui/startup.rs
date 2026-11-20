//! Storage, schema upgrades and legacy preference import run before service
//! attachment on a blocking worker. Platform adapters attach on the UI thread.
use super::*;
use crate::error::{AppError, AppResult};

pub(super) struct Startup {
    runtime: AppRuntime,
    messages: tokio::sync::mpsc::UnboundedReceiver<UiMessage>,
    preferences: Preferences,
    logs: diagnostics::LogBook,
    statistics_enabled: bool,
}
impl Startup {
    pub fn load(preview: bool) -> AppResult<Self> {
        let (runtime, messages) = AppRuntime::open_with_preview(preview)?;
        let host = runtime.host();
        let preferences = Preferences::load(&host);
        let logs = diagnostics::LogBook::load(host.clone(), preferences.logs_enabled);
        let statistics_enabled = host
            .state::<AppState>()
            .db
            .get_setting(crate::db::SETTING_LISTEN_STATS_ENABLED)?
            .as_deref()
            != Some("false");
        Ok(Self {
            runtime,
            messages,
            preferences,
            logs,
            statistics_enabled,
        })
    }
}
impl PlayerApp {
    pub(super) fn initialize(&mut self, preview: bool, window: &Window, cx: &mut Context<Self>) {
        if self.busy > 0 {
            return;
        }
        self.busy += 1;
        self.error = None;
        let epoch = self.epoch;
        let work = crate::runtime::spawn_blocking(move || Startup::load(preview));
        cx.spawn_in(window, async move |view, cx| {
            let result = work
                .await
                .map_err(|error| AppError::msg(format!("Startup worker failed: {error}")))
                .and_then(|v| v);
            let _ = view.update_in(cx, |app, window, cx| {
                if app.epoch != epoch {
                    return;
                }
                app.busy = app.busy.saturating_sub(1);
                app.attach_runtime(result, preview, window, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn attach_runtime(
        &mut self,
        startup: AppResult<Startup>,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Startup {
            mut runtime,
            mut messages,
            preferences,
            logs,
            statistics_enabled,
        } = match startup {
            Ok(startup) => startup,
            Err(error) => {
                self.phase = Phase::Loading;
                self.error = Some(error.to_string());
                return;
            }
        };
        let host = runtime.host();
        #[cfg(target_os = "windows")]
        if let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
            if let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() {
                host.set_native_window_handle(handle.hwnd.get() as usize);
            }
        }
        self.preferences = preferences;
        self.logs = logs;
        self.settings.update(cx, |feature, cx| {
            feature.statistics_enabled = statistics_enabled;
            cx.notify();
        });
        let mut selected = PlaylistId::parse(&self.preferences.selected);
        if !statistics_enabled
            && matches!(&selected, PlaylistId::Builtin(id) if id == "popular" || id == "recent")
        {
            selected = PlaylistId::All;
            self.preferences.selected = "all".into();
        }
        self.library.update(cx, |library, cx| {
            library.select(selected);
            cx.notify();
        });
        self.host = Some(host);
        if !preview {
            runtime.start();
        }
        self.runtime = Some(runtime);
        cx.spawn_in(window, async move |view, cx| {
            while let Some(message) = messages.recv().await {
                match message {
                    UiMessage::MainThread(work) => {
                        if view.update(cx, |_, _| ()).is_err() {
                            break;
                        }
                        work();
                    }
                    UiMessage::Event(event) => {
                        if view
                            .update_in(cx, |app, window, cx| {
                                app.event(*event, cx);
                                app.update_effects(window, cx);
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        })
        .detach();
        if preview {
            self.set_preview(cx);
            self.update_effects(window, cx);
            if std::env::args().any(|arg| arg == "--preview-compact") {
                window.resize(size(px(390.), px(844.)));
            }
            self.start_preview_performance(window, cx);
        } else {
            self.bootstrap(cx);
        }
    }
}
