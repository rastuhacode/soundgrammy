//! One ordered UI command port per account/view lifetime. Preparing a slow
//! playback command cannot let later pause, seek, or volume intent overtake it.
use crate::{
    audio::{Control, Snapshot},
    runtime::AppHost,
    state::AppState,
};
use futures_util::FutureExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot};
type Reply = oneshot::Sender<Result<Snapshot, String>>;
pub(super) struct CommandPort {
    sender: mpsc::UnboundedSender<(Control, Reply)>,
    active: Arc<AtomicBool>,
}
impl CommandPort {
    pub fn new(host: AppHost, active: Arc<AtomicBool>) -> Self {
        let db = host.state::<AppState>().db.clone();
        let generation = async move {
            crate::runtime::spawn_blocking(move || db.account_generation())
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
        }
        .boxed()
        .shared();
        Self::with_handler(move |control| {
            let host = host.clone();
            let generation = generation.clone();
            let active = active.clone();
            async move {
                let generation = generation.await?;
                if !active.load(Ordering::Acquire) {
                    return Err("Account changed".into());
                }
                let Some(state) = host.try_state::<AppState>() else {
                    return Err("Application closed".into());
                };
                if state
                    .db
                    .account_generation()
                    .map_err(|error| error.to_string())?
                    != generation
                {
                    return Err("Account changed".into());
                }
                state
                    .audio
                    .command(host, control)
                    .await
                    .map(|snapshot| snapshot.for_display())
            }
        })
    }
    fn with_handler<F, Fut>(handler: F) -> Self
    where
        F: FnMut(Control) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Snapshot, String>> + Send + 'static,
    {
        let (port, worker) = Self::worker(handler);
        crate::runtime::spawn(worker);
        port
    }

    #[cfg(test)]
    pub(super) fn with_test_handler<F, Fut>(cx: &gpui_kit::App, handler: F) -> Self
    where
        F: FnMut(Control) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Snapshot, String>> + Send + 'static,
    {
        let (port, worker) = Self::worker(handler);
        cx.background_executor().spawn(worker).detach();
        port
    }

    fn worker<F, Fut>(mut handler: F) -> (Self, impl std::future::Future<Output = ()> + Send)
    where
        F: FnMut(Control) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Snapshot, String>> + Send + 'static,
    {
        let (sender, mut receiver) = mpsc::unbounded_channel::<(Control, Reply)>();
        let active = Arc::new(AtomicBool::new(true));
        let current = active.clone();
        let worker = async move {
            while let Some((control, reply)) = receiver.recv().await {
                let result = if !current.load(Ordering::Acquire) {
                    Err("Playback view closed".into())
                } else {
                    handler(control).await
                };
                let _ = reply.send(result);
            }
        };
        (Self { sender, active }, worker)
    }
    pub fn send(&self, control: Control) -> oneshot::Receiver<Result<Snapshot, String>> {
        let (send, receive) = oneshot::channel();
        let _ = self.sender.send((control, send));
        receive
    }
}
impl Drop for CommandPort {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[tokio::test]
    async fn slow_playback_preparation_cannot_be_overtaken_by_later_intent() {
        let gate = Arc::new(tokio::sync::Notify::new());
        let began = Arc::new(tokio::sync::Notify::new());
        let order = Arc::new(Mutex::new(vec![]));
        let port = CommandPort::with_handler({
            let (gate, began, order) = (gate.clone(), began.clone(), order.clone());
            move |control| {
                let (gate, began, order) = (gate.clone(), began.clone(), order.clone());
                async move {
                    let Control::Volume(value) = control else {
                        panic!("test command")
                    };
                    if value == 1. {
                        began.notify_one();
                        gate.notified().await;
                    }
                    order.lock().unwrap().push(value);
                    Ok(Snapshot::default())
                }
            }
        });
        let first = port.send(Control::Volume(1.));
        began.notified().await;
        let second = port.send(Control::Volume(2.));
        let third = port.send(Control::Volume(3.));
        gate.notify_one();
        for reply in [first, second, third] {
            reply.await.unwrap().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), vec![1., 2., 3.]);
    }
    #[tokio::test]
    async fn closed_account_port_discards_queued_commands() {
        let gate = Arc::new(tokio::sync::Notify::new());
        let began = Arc::new(tokio::sync::Notify::new());
        let port = CommandPort::with_handler({
            let (gate, began) = (gate.clone(), began.clone());
            move |_| {
                let (gate, began) = (gate.clone(), began.clone());
                async move {
                    began.notify_one();
                    gate.notified().await;
                    Ok(Snapshot::default())
                }
            }
        });
        let first = port.send(Control::Play);
        began.notified().await;
        let queued = port.send(Control::Pause);
        drop(port);
        gate.notify_one();
        first.await.unwrap().unwrap();
        assert!(queued.await.unwrap().unwrap_err().contains("closed"));
    }
}
