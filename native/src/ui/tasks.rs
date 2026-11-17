use super::*;
use crate::error::{AppError, AppResult};

impl PlayerApp {
    pub(super) fn task<T, F, E>(
        &mut self,
        cx: &mut Context<Self>,
        future: F,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        F: Future<Output = Result<T, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        self.task_inner(cx, future, apply, false);
    }

    pub(super) fn quiet_task<T, F, E>(
        &mut self,
        cx: &mut Context<Self>,
        future: F,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        F: Future<Output = Result<T, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        self.task_inner(cx, future, apply, true);
    }

    pub(super) fn task_inner<T, F, E>(
        &mut self,
        cx: &mut Context<Self>,
        future: F,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        quiet: bool,
    ) where
        T: Send + 'static,
        F: Future<Output = Result<T, E>> + Send + 'static,
        E: std::fmt::Display + Send + 'static,
    {
        let epoch = self.epoch;
        if !quiet {
            self.busy += 1;
            self.error = None;
        }
        let job = crate::runtime::spawn(async move { future.await.map_err(|e| e.to_string()) });
        let window = self.window;
        cx.spawn(async move |view, cx| {
            let result = job
                .await
                .map_err(|_| "A background operation failed.".to_owned())
                .and_then(|v| v);
            let _ = window.update(cx, |_, window, cx| {
                let _ = view.update(cx, |app, cx| {
                    if app.epoch != epoch {
                        return;
                    }
                    if !quiet {
                        app.busy = app.busy.saturating_sub(1);
                    }
                    match result {
                        Ok(value) => apply(app, value, cx),
                        Err(error) => {
                            app.logs.push(error.clone());
                            if !quiet {
                                app.error = Some(error);
                            }
                        }
                    }
                    app.update_effects(window, cx);
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }

    /// Serialize account writes and reject queued intent after logout.
    pub(super) fn mutation<T, F, Fut>(
        &mut self,
        cx: &mut Context<Self>,
        work: F,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        F: FnOnce(AppHost) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<T>> + Send + 'static,
    {
        let host = self.host();
        let db = host.state::<AppState>().db.clone();
        let generation = crate::runtime::spawn_blocking(move || db.account_generation());
        let active = self.account_active.clone();
        let lock = self.mutations.clone();
        self.task(
            cx,
            async move {
                let _guard = lock.lock().await;
                let expected = generation
                    .await
                    .map_err(|error| AppError::msg(format!("Account worker failed: {error}")))??;
                if !active.load(std::sync::atomic::Ordering::Acquire)
                    || host.state::<AppState>().db.account_generation()? != expected
                {
                    return Err(AppError::NotAuthorized);
                }
                work(host).await
            },
            apply,
        );
    }
}
