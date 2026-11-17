//! Native application lifetime, typed notifications, and platform services.
//! Services never depend on a renderer or on a WebView. Handles are weak so
//! streams and background jobs cannot keep a closed application alive.
mod persistence;

use crate::{
    config::Config,
    db::Db,
    error::{AppError, AppResult},
    state::AppState,
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    future::Future,
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock, Weak},
};
use tokio::{runtime::Runtime, sync::mpsc, task::JoinHandle};

#[cfg(target_os = "android")]
static MOBILE_RESOURCES: OnceLock<Arc<Resources>> = OnceLock::new();

#[cfg(target_os = "android")]
pub(crate) fn request_mobile_back() -> bool {
    MOBILE_RESOURCES.get().is_some_and(|r| {
        r.notifications
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .send(UiMessage::Event(Box::new(Event::Back)))
            .is_ok()
    })
}

static EXECUTOR: OnceLock<Runtime> = OnceLock::new();
fn executor() -> &'static Runtime {
    EXECUTOR.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("soundgrammy-async")
            .build()
            .expect("failed to start native async runtime")
    })
}
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    executor().spawn(future)
}
pub fn spawn_blocking<F, R>(work: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    executor().spawn_blocking(work)
}

/// Domain notifications are exhaustive Rust values, independent of GPUI.
#[derive(Clone)]
pub(crate) enum Event {
    #[cfg(target_os = "android")]
    Back,
    AuthRevoked,
    AudioState(crate::audio::Snapshot),
    LibraryBatch(crate::db::LibraryBatch),
    SyncStarted,
    SyncFinished,
    SyncFailed(crate::telegram::saved_music::SyncError),
    SyncProgress(crate::telegram::saved_music::SyncProgress),
    DownloadProgress(crate::streaming::DownloadProgress),
    CacheChanged(crate::cache::CacheChanged),
    ListenStats(crate::db::TrackListenStats),
    LastFmStatus(crate::lastfm::models::LastFmStatus),
    ArtworkChanged(crate::cache::media_art::ArtworkChanged),
    CacheProgress(crate::cache::CacheTracksProgress),
    ExportProgress(crate::export::PlaylistDownloadProgress),
}
pub(crate) enum UiMessage {
    Event(Box<Event>),
    MainThread(Box<dyn FnOnce() + Send>),
}
struct Resources {
    values: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    notifications: RwLock<mpsc::UnboundedSender<UiMessage>>,
    downloads: PathBuf,
    native_window: std::sync::atomic::AtomicUsize,
    settings_writer: Option<persistence::SettingsWriter>,
    _preview_directory: Option<tempfile::TempDir>,
}
#[derive(Clone)]
pub struct AppHost {
    resources: Weak<Resources>,
}
impl AppHost {
    pub fn try_state<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.resources
            .upgrade()?
            .values
            .read()
            .ok()?
            .get(&TypeId::of::<T>())?
            .clone()
            .downcast()
            .ok()
    }
    pub fn state<T: Any + Send + Sync>(&self) -> Arc<T> {
        self.try_state()
            .expect("native application state unavailable")
    }
    pub fn manage<T: Any + Send + Sync>(&self, state: T) -> bool {
        let Some(resources) = self.resources.upgrade() else {
            return false;
        };
        let mut values = resources.values.write().unwrap_or_else(|e| e.into_inner());
        if values.contains_key(&TypeId::of::<T>()) {
            return false;
        }
        values.insert(TypeId::of::<T>(), Arc::new(state));
        true
    }
    pub(crate) fn emit(&self, event: Event) -> AppResult<()> {
        self.send(UiMessage::Event(Box::new(event)))
    }
    pub(crate) fn persist_setting<T: serde::Serialize + Send + 'static>(
        &self,
        key: &'static str,
        value: T,
    ) -> AppResult<()> {
        let resources = self
            .resources
            .upgrade()
            .ok_or_else(|| AppError::msg("Application closed"))?;
        resources
            .settings_writer
            .as_ref()
            .ok_or_else(|| AppError::msg("Settings writer unavailable"))?
            .save(key, value)
    }
    fn send(&self, message: UiMessage) -> AppResult<()> {
        self.resources
            .upgrade()
            .ok_or_else(|| AppError::msg("Application closed"))?
            .notifications
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .send(message)
            .map_err(|_| AppError::msg("Application closed"))
    }
    pub fn run_on_main_thread(&self, work: impl FnOnce() + Send + 'static) -> AppResult<()> {
        self.send(UiMessage::MainThread(Box::new(work)))
    }
    pub fn download_dir(&self) -> AppResult<PathBuf> {
        self.resources
            .upgrade()
            .map(|r| r.downloads.clone())
            .ok_or_else(|| AppError::msg("Application closed"))
    }
    pub fn open_url(&self, url: &str) -> AppResult<()> {
        #[cfg(target_os = "android")]
        {
            gpui_mobile::packages::url_launcher::launch_url(url)
                .map_err(AppError::msg)
                .and_then(|opened| {
                    if opened {
                        Ok(())
                    } else {
                        Err(AppError::msg("The page could not be opened."))
                    }
                })
        }
        #[cfg(not(target_os = "android"))]
        {
            open::that_detached(url).map_err(|_| AppError::msg("The page could not be opened."))
        }
    }
    pub fn set_native_window_handle(&self, handle: usize) {
        if let Some(r) = self.resources.upgrade() {
            r.native_window
                .store(handle, std::sync::atomic::Ordering::Release);
        }
    }
    pub fn native_window_handle(&self) -> Option<usize> {
        self.resources
            .upgrade()
            .map(|r| r.native_window.load(std::sync::atomic::Ordering::Acquire))
            .filter(|v| *v != 0)
    }
}

/// Owns resources until audio and background services have been shut down.
pub struct AppRuntime {
    resources: Arc<Resources>,
    worker: Option<JoinHandle<()>>,
}
impl AppRuntime {
    pub(crate) fn open_with_preview(
        preview: bool,
    ) -> AppResult<(Self, mpsc::UnboundedReceiver<UiMessage>)> {
        #[cfg(target_os = "android")]
        if let Some(resources) = MOBILE_RESOURCES.get() {
            let (sender, receiver) = mpsc::unbounded_channel();
            *resources
                .notifications
                .write()
                .unwrap_or_else(|e| e.into_inner()) = sender;
            let runtime = Self {
                resources: resources.clone(),
                worker: None,
            };
            runtime.host().state::<AppState>().audio.page_loading();
            return Ok((runtime, receiver));
        }

        let configuration = application_config();
        let isolated = std::env::var_os("SOUNDGRAMMY_DATA_DIR").is_some();
        let identifier = if preview || isolated {
            "com.soundgrammy.app.sandbox"
        } else {
            configuration["identifier"]
                .as_str()
                .expect("native app identity")
        };
        // Every preview/test gets its own database, including during schema upgrades.
        let preview_directory = if preview && !isolated {
            Some(
                tempfile::Builder::new()
                    .prefix("soundgrammy-preview-")
                    .tempdir()?,
            )
        } else {
            None
        };
        let (data, cache, downloads) = if let Some(directory) = &preview_directory {
            let root = directory.path();
            (
                root.join("data"),
                root.join("cache"),
                root.join("Downloads"),
            )
        } else {
            application_paths(identifier)?
        };
        #[cfg(target_os = "android")]
        crate::android_keyring::initialize_store().map_err(AppError::msg)?;
        std::fs::create_dir_all(&data)?;
        std::fs::create_dir_all(&cache)?;
        crate::session::initialize_namespace(&crate::session::app_namespace(identifier, None));
        #[cfg(target_os = "android")]
        crate::session::repair_android_restored_session(&data)?;
        let config = if preview {
            Config {
                api_id: 0,
                api_hash: String::new(),
                lastfm: None,
            }
        } else {
            Config::load()?
        };
        let db = Db::open(&data.join("library.db"))?;
        let (notifications, receiver) = mpsc::unbounded_channel();
        let resources = Arc::new(Resources {
            values: Default::default(),
            notifications: RwLock::new(notifications),
            downloads,
            native_window: Default::default(),
            settings_writer: Some(persistence::SettingsWriter::new(db.clone())?),
            _preview_directory: preview_directory,
        });
        #[cfg(target_os = "android")]
        let _ = MOBILE_RESOURCES.set(resources.clone());
        let runtime = Self {
            resources,
            worker: None,
        };
        let host = runtime.host();
        host.manage(AppState::new(config, db, None, data, cache, false, None));
        host.manage(crate::display_wake::DisplayWakeState::new()?);
        Ok((runtime, receiver))
    }
    pub fn host(&self) -> AppHost {
        AppHost {
            resources: Arc::downgrade(&self.resources),
        }
    }
    /// Invoke after the shell has supplied its native window handle, on the main thread.
    pub fn start(&mut self) {
        if self.worker.is_some()
            || self
                .host()
                .try_state::<crate::audio::media::Bridge>()
                .is_some()
        {
            return;
        }
        let host = self.host();
        crate::audio::media::initialize(&host);
        let maintenance = host.clone();
        spawn(async move {
            let state = maintenance.state::<AppState>();
            if let Err(error) = crate::cache::enforce_ttl(&state, &maintenance).await {
                tracing::warn!(%error, "Cache maintenance failed");
            }
        });
        self.worker = Some(spawn(crate::lastfm::LastFmService::worker_loop(host)));
    }
}
impl Drop for AppRuntime {
    fn drop(&mut self) {
        // Android Activity recreation must not destroy the process-owned player.
        #[cfg(not(target_os = "android"))]
        self.shutdown();
    }
}
#[cfg(not(target_os = "android"))]
impl AppRuntime {
    fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
        if let Some(state) = self.host().try_state::<AppState>() {
            state.audio.shutdown();
        }
        crate::audio::media::shutdown(&self.host());
    }
}

pub(crate) fn application_config() -> serde_json::Value {
    let mut configuration: serde_json::Value =
        serde_json::from_str(include_str!("../../native-app.json"))
            .expect("native app configuration");
    if cfg!(debug_assertions) {
        let development: serde_json::Value =
            serde_json::from_str(include_str!("../../native-app.dev.json"))
                .expect("development configuration");
        for (key, value) in development.as_object().unwrap() {
            configuration[key] = value.clone();
        }
    }
    configuration
}

fn application_paths(_identifier: &str) -> AppResult<(PathBuf, PathBuf, PathBuf)> {
    if let Some(root) = std::env::var_os("SOUNDGRAMMY_DATA_DIR") {
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return Err(AppError::msg(
                "SOUNDGRAMMY_DATA_DIR must be an absolute path",
            ));
        }
        return Ok((
            root.join("data"),
            root.join("cache"),
            root.join("Downloads"),
        ));
    }
    #[cfg(target_os = "android")]
    {
        crate::android_keyring::application_paths().map_err(AppError::msg)
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok((
            dirs::data_dir()
                .ok_or_else(|| AppError::msg("Application data directory unavailable"))?
                .join(_identifier),
            dirs::cache_dir()
                .ok_or_else(|| AppError::msg("Application cache directory unavailable"))?
                .join(_identifier),
            dirs::download_dir().unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("Downloads")
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn resources() -> (Arc<Resources>, AppHost, mpsc::UnboundedReceiver<UiMessage>) {
        let (notifications, receiver) = mpsc::unbounded_channel();
        let r = Arc::new(Resources {
            values: Default::default(),
            notifications: RwLock::new(notifications),
            downloads: PathBuf::from("/tmp"),
            native_window: Default::default(),
            settings_writer: None,
            _preview_directory: None,
        });
        let h = AppHost {
            resources: Arc::downgrade(&r),
        };
        (r, h, receiver)
    }
    #[test]
    fn handles_do_not_keep_application_or_account_state_alive() {
        let (owner, host, _) = resources();
        host.manage(42_u64);
        assert_eq!(*host.state::<u64>(), 42);
        assert!(!host.manage(7_u64));
        drop(owner);
        assert!(host.try_state::<u64>().is_none());
        assert!(host.emit(Event::SyncStarted).is_err());
    }
    #[test]
    fn notifications_preserve_types_and_order_and_main_thread_work_is_deferred() {
        let (_owner, host, mut receiver) = resources();
        host.emit(Event::SyncStarted).unwrap();
        let changed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let target = changed.clone();
        host.run_on_main_thread(move || target.store(true, std::sync::atomic::Ordering::Release))
            .unwrap();
        assert!(!changed.load(std::sync::atomic::Ordering::Acquire));
        let UiMessage::Event(event) = receiver.try_recv().unwrap() else {
            panic!("event expected")
        };
        assert!(matches!(*event, Event::SyncStarted));
        let UiMessage::MainThread(work) = receiver.try_recv().unwrap() else {
            panic!("work expected")
        };
        work();
        assert!(changed.load(std::sync::atomic::Ordering::Acquire));
    }
}
