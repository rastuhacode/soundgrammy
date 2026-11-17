//! Window composition and shared intent. Feature data and inputs have retained
//! GPUI owners; services and the native player keep their existing lifetimes.
use super::*;
pub(super) struct PlayerApp {
    pub(super) window: AnyWindowHandle,
    pub(super) preview: bool,
    pub(super) updating_effects: bool,
    pub(super) clear_track_search: bool,
    pub(super) reset_inputs: bool,
    pub(super) pending_full_sync: bool,
    pub(super) panel_stack: Vec<Panel>,
    pub(super) retained_modal: Option<Panel>,
    pub(super) modal_progress: f32,
    pub(super) bounce_ready_at: Option<std::time::Instant>,
    #[cfg(test)]
    pub(super) inactive_row_renders: std::cell::Cell<usize>,
    pub(super) track_equalizer: Entity<motion::Equalizer>,
    pub(super) wake_requests: Option<tokio::sync::mpsc::UnboundedSender<bool>>,
    pub(super) seek_preview: Option<(f32, f64)>,
    pub(super) seek_pointer: Option<Point<Pixels>>,
    pub(super) pending_back: bool,

    pub(super) download_results: std::collections::VecDeque<crate::export::PlaylistDownloadResult>,
    pub(super) download_sections: std::collections::HashSet<&'static str>,
    pub(super) queue_save_scope: QueueSaveScope,
    pub(super) queue_scope_focus: [FocusHandle; 3],
    pub(super) playlist_sort_focus: [FocusHandle; 3],
    pub(super) loading_started: Option<std::time::Instant>,
    pub(super) loading_indicator_task: Option<Task<()>>,
    pub(super) new_playlist_import: bool,
    pub(super) new_playlist_tracks: Vec<i64>,
    pub(super) touch_press: Option<(usize, Point<Pixels>, u64)>,
    pub(super) touch_generation: u64,
    pub(super) suppress_touch_click: Option<usize>,
    pub(super) queue_list: ListState,
    pub(super) queue_items: Vec<QueueItem>,
    pub(super) hq_thumbs: std::collections::HashSet<i64>,
    pub(super) thumb_loading: std::collections::HashSet<i64>,
    pub(super) metadata: Option<(i64, commands::TrackMetadata)>,
    pub(super) volume_wheel: f64,
    pub(super) volume_hover: bool,
    pub(super) volume_pointer_over: [bool; 2],
    pub(super) volume_dragging: bool,
    pub(super) volume_close_task: Option<Task<()>>,
    pub(super) volume_exact_open: bool,
    pub(super) volume_focus: FocusHandle,
    pub(super) runtime: Option<AppRuntime>,
    pub(super) host: Option<AppHost>,
    pub(super) phase: Phase,
    pub(super) epoch: u64,
    pub(super) account_active: Arc<std::sync::atomic::AtomicBool>,
    pub(super) hover_title_epoch: std::rc::Rc<std::cell::Cell<u64>>,
    pub(super) user: Option<AuthUser>,
    pub(super) library: Entity<Library>,
    pub(super) settings: Entity<settings::SettingsState>,
    pub(super) preferences: Preferences,
    pub(super) playback: Entity<audio::Snapshot>,
    pub(super) sign_in: Entity<login::LoginView>,
    pub(super) inputs: Inputs,
    pub(super) focus: FocusHandle,
    pub(super) modal_focus: FocusHandle,
    pub(super) panel_was_open: bool,
    pub(super) panel_return_focus: Option<FocusHandle>,
    pub(super) seek_slider: Entity<SliderState>,
    pub(super) synced_seek_percent: Option<f32>,
    pub(super) synced_volume_percent: Option<f32>,
    pub(super) volume_slider: Entity<SliderState>,
    pub(super) subscriptions: Vec<Subscription>,
    pub(super) panel: Option<Panel>,
    pub(super) busy: usize,
    pub(super) error: Option<String>,
    pub(super) notice: Option<String>,
    pub(super) syncing: bool,
    pub(super) sync_progress: Option<(i64, i64)>,
    pub(super) online: bool,
    pub(super) compact_playlist: bool,
    pub(super) fullscreen: bool,
    pub(super) fullscreen_request: u64,
    pub(super) fullscreen_activity: std::time::Instant,
    pub(super) fullscreen_idle_task: Option<Task<()>>,
    pub(super) fullscreen_cursor_hidden: bool,
    #[cfg(target_os = "macos")]
    pub(super) cursor_hide_mode_before_fullscreen: Option<gpui_kit::CursorHideMode>,
    pub(super) fullscreen_visual: Entity<artwork::FullscreenVisual>,
    pub(super) artwork_cache: Entity<artwork::ThumbnailCache>,
    pub(super) fullscreen_pointer: Option<Point<Pixels>>,
    pub(super) selection_mode: bool,
    pub(super) artwork_revision: u64,
    pub(super) thumbs: HashMap<i64, Option<std::path::PathBuf>>,
    pub(super) download_progress: HashMap<i64, f64>,
    pub(super) avatar: Option<std::path::PathBuf>,
    pub(super) jobs: HashMap<String, String>,
    pub(super) logs: diagnostics::LogBook,

    pub(super) mutations: Arc<tokio::sync::Mutex<()>>,
    pub(super) command_port: Option<transport::CommandPort>,
    pub(super) reconnect_running: bool,
    pub(super) bounce_profile: Option<Arc<artwork::BounceProfile>>,
    pub(super) bounce_track: Option<i64>,
    pub(super) sort_drag: Option<views::SortDrag>,
    pub(super) scrubbing: Option<f64>,
    pub(super) playlist_search: String,
    pub(super) previous_volume: f64,
    pub(super) palette: artwork::Palette,
    pub(super) playing_artwork_path: Option<std::path::PathBuf>,
    pub(super) playing_artwork: Option<Arc<RenderImage>>,
    pub(super) palette_path: Option<std::path::PathBuf>,
    pub(super) preparing_artwork_path: Option<std::path::PathBuf>,
    pub(super) prepared_artwork_track: Option<i64>,
    pub(super) fullscreen_cover: Option<Arc<RenderImage>>,
    pub(super) pending_fullscreen: bool,
    pub(super) fullscreen_backdrop: artwork::Background,
}
pub(super) struct Inputs {
    pub(super) search: Entity<InputState>,
    pub(super) playlist_search: Entity<InputState>,
    pub(super) name: Entity<InputState>,

    pub(super) exact_volume: Entity<InputState>,
}
impl PlayerApp {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_inner(
            window,
            cx,
            std::env::args().any(|arg| arg == "--preview" || arg == "--preview-fixture"),
        )
    }

    pub(super) fn new_inner(window: &mut Window, cx: &mut Context<Self>, preview: bool) -> Self {
        let input = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let inputs = Inputs {
            search: input("Search tracks", window, cx),
            playlist_search: input("Search playlists", window, cx),
            name: input("My playlist", window, cx),
            exact_volume: input("0–100", window, cx),
        };
        let seek_slider = cx.new(|_| SliderState::new().min(0.).max(100.).step(0.01));
        let volume_slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(100.)
        });
        let artwork_cache = artwork::ThumbnailCache::new(cx);
        let controller = cx.weak_entity();
        let mut app = Self {
            window: window.window_handle(),
            preview,
            updating_effects: false,
            clear_track_search: false,
            reset_inputs: false,
            pending_full_sync: false,
            panel_stack: vec![],
            retained_modal: None,
            modal_progress: 1.,
            bounce_ready_at: None,
            #[cfg(test)]
            inactive_row_renders: Default::default(),
            track_equalizer: cx.new(|_| motion::Equalizer::new(false)),
            wake_requests: None,
            seek_preview: None,
            seek_pointer: None,
            pending_back: false,

            download_results: Default::default(),
            download_sections: Default::default(),
            queue_save_scope: QueueSaveScope::Full,
            queue_scope_focus: std::array::from_fn(|_| cx.focus_handle()),
            playlist_sort_focus: std::array::from_fn(|_| cx.focus_handle()),
            loading_started: None,
            loading_indicator_task: None,
            new_playlist_import: false,
            new_playlist_tracks: vec![],
            touch_press: None,
            touch_generation: 0,
            suppress_touch_click: None,
            queue_list: ListState::new(0, ListAlignment::Top, px(150.)),
            queue_items: vec![],
            hq_thumbs: Default::default(),
            thumb_loading: Default::default(),
            metadata: None,
            volume_wheel: 0.,
            volume_hover: false,
            volume_pointer_over: [false; 2],
            volume_dragging: false,
            volume_close_task: None,
            volume_exact_open: false,
            volume_focus: cx.focus_handle(),
            runtime: None,
            host: None,
            phase: Phase::Loading,
            epoch: 0,
            account_active: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            hover_title_epoch: Default::default(),
            user: None,
            library: cx.new(|_| Library::default()),
            settings: cx.new(|cx| settings::SettingsState::new(window, cx)),
            preferences: Default::default(),
            playback: cx.new(|_| audio::Snapshot::default()),
            sign_in: cx.new(|feature_cx| login::LoginView::new(controller, window, feature_cx)),
            inputs,
            focus: cx.focus_handle(),
            modal_focus: cx.focus_handle(),
            panel_was_open: false,
            panel_return_focus: None,
            seek_slider,
            synced_seek_percent: None,
            synced_volume_percent: None,
            volume_slider,
            subscriptions: vec![],
            panel: None,
            busy: 0,
            error: None,
            notice: None,
            syncing: false,
            sync_progress: None,
            online: false,
            compact_playlist: false,
            fullscreen: false,
            fullscreen_request: 0,
            fullscreen_activity: cx.background_executor().now(),
            fullscreen_idle_task: None,
            fullscreen_cursor_hidden: false,
            #[cfg(target_os = "macos")]
            cursor_hide_mode_before_fullscreen: None,
            fullscreen_visual: cx.new(|_| artwork::FullscreenVisual::new(artwork_cache.clone())),
            artwork_cache,
            fullscreen_pointer: None,
            selection_mode: false,
            artwork_revision: 0,
            thumbs: HashMap::new(),
            download_progress: Default::default(),
            avatar: None,
            jobs: HashMap::new(),
            logs: Default::default(),

            mutations: Default::default(),
            command_port: None,
            reconnect_running: false,
            bounce_profile: None,
            bounce_track: None,
            sort_drag: None,
            scrubbing: None,
            playlist_search: String::new(),
            previous_volume: 25.,
            palette: Default::default(),
            playing_artwork_path: None,
            playing_artwork: None,
            palette_path: None,
            preparing_artwork_path: None,
            prepared_artwork_track: None,
            fullscreen_cover: None,
            pending_fullscreen: false,
            fullscreen_backdrop: Default::default(),
        };
        app.subscriptions
            .push(cx.observe_window_visibility(window, |_, _, _, cx| cx.notify()));
        app.subscriptions
            .push(cx.observe_window_activation(window, |app, window, cx| {
                if app.fullscreen && window.is_window_active() {
                    app.show_fullscreen_controls(window, cx);
                } else {
                    cx.notify();
                }
            }));
        app.subscriptions
            .push(cx.subscribe(&app.inputs.search, |app, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    app.library.clone().update(cx, |library, cx| {
                        library.search = input.read(cx).value().to_string();
                        cx.notify();
                    });
                    cx.notify();
                }
            }));
        app.subscriptions.push(cx.subscribe(
            &app.inputs.playlist_search,
            |app, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    app.playlist_search = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        ));
        app.subscriptions
            .push(cx.subscribe(&app.seek_slider, |app, _, event, cx| {
                match event {
                    SliderEvent::Change(value) if value.start().is_finite() => {
                        app.scrubbing = Some(value.start() as f64);
                    }
                    SliderEvent::Release(value) => {
                        app.scrubbing = None;
                        if value.start().is_finite() {
                            app.seek(
                                value.start() as f64 / 100.
                                    * app.playback.read(cx).duration_seconds,
                                cx,
                            );
                        }
                    }
                    _ => {}
                }
                cx.notify();
            }));
        app.subscriptions
            .push(cx.subscribe(&app.volume_slider, |app, _, event, cx| {
                if let SliderEvent::Change(value) = event {
                    if value.start().is_finite() {
                        app.set_volume(value.start() as f64, cx);
                    } else {
                        cx.notify();
                    }
                }
            }));
        app.subscriptions.push(
            cx.on_focus_in(&app.volume_focus, window, |app, window, cx| {
                app.update_volume_hover(window, cx)
            }),
        );
        app.subscriptions.push(
            cx.on_focus_out(&app.volume_focus, window, |app, _, window, cx| {
                app.update_volume_hover(window, cx)
            }),
        );
        app.subscriptions.push(
            cx.subscribe(&app.inputs.exact_volume, |app, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
                if (app.volume_exact_open || matches!(app.panel, Some(Panel::Volume)))
                    && matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur)
                {
                    if let Ok(value) = finite_nonnegative(input.read(cx).value().as_ref(), "Volume")
                    {
                        if value <= 100. {
                            app.set_volume(value, cx);
                            app.volume_exact_open = false;
                        }
                    }
                }
            }),
        );
        app.subscriptions
            .push(cx.subscribe(&app.inputs.name, |app, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
                if matches!(event, InputEvent::PressEnter { .. })
                    && matches!(
                        app.panel,
                        Some(Panel::NewPlaylist | Panel::RenamePlaylist(_) | Panel::SaveQueue)
                    )
                    && app.busy == 0
                    && !app.inputs.name.read(cx).value().trim().is_empty()
                {
                    app.save_playlist(cx);
                }
            }));
        #[cfg(test)]
        if preview {
            app.attach_runtime(startup::Startup::load(true), true, window, cx);
        } else {
            app.initialize(false, window, cx);
        }
        #[cfg(not(test))]
        app.initialize(preview, window, cx);
        app.focus.focus(window, cx);
        app
    }

    pub(super) fn settings_inputs(&self, cx: &App) -> settings::SettingsInputs {
        self.settings.read(cx).inputs.clone()
    }
    pub(super) fn login_inputs(&self, cx: &App) -> login::LoginInputs {
        self.sign_in.read(cx).inputs.clone()
    }

    pub(super) fn host(&self) -> AppHost {
        self.host.clone().expect("application runtime unavailable")
    }

    pub(super) fn save_preferences(&mut self) {
        self.logs.set_enabled(self.preferences.logs_enabled);
        if let Some(host) = &self.host {
            if let Err(e) = self.preferences.save(host) {
                self.logs.push(e.to_string());
            }
        }
    }
}
