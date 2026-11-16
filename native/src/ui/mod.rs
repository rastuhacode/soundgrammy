//! Native GPUI interface. Domain work is dispatched to the Rust services;
//! account-scoped results are discarded after logout or account replacement.
mod actions;
mod artwork;
mod bounce;
mod diagnostics;
mod dialogs;
mod file_dialog;
mod login;
mod menus;
mod model;
mod motion;
mod overlays;
mod preferences;
mod preview;
mod settings;
#[cfg(test)]
mod tests;
mod transport;
mod views;
use crate::{
    audio::{
        self,
        session::{Command, Queue, Source, SourceType},
    },
    commands,
    runtime::{AppHost, AppRuntime, Event, UiMessage},
    state::AppState,
    telegram::auth::{AuthOutcome, AuthUser, PhoneSendCodeOutcome, QrOutcome},
};
use gpui_kit::component::{
    input::{InputEvent, InputState, TextareaState},
    slider::{SliderEvent, SliderState},
    Theme, ThemeMode,
};
use gpui_kit::{prelude::*, *};
use model::*;
use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};
use views::HoverTitle;

actions!(
    soundgrammy,
    [
        Quit,
        TogglePlayback,
        NextTrack,
        PreviousTrack,
        SeekForward,
        SeekBackward,
        Escape,
        OpenSettings,
        ToggleFullscreen,
        About,
        HideApp,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        WindowFullscreen
    ]
);

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(launch);
}

pub(super) fn launch(cx: &mut App) {
    gpui_kit::init(cx);
    #[cfg(target_os = "macos")]
    set_application_icon();
    configure_theme(cx);
    bind_shortcuts(cx);
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    configure_menus(cx);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
    let config = crate::runtime::application_config();
    let dimensions = &config["window"];
    let dimension = |key: &str| dimensions[key].as_f64().expect("window dimension") as f32;
    let bounds = Bounds::centered(
        None,
        size(px(dimension("width")), px(dimension("height"))),
        cx,
    );
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(dimension("minWidth")), px(dimension("minHeight")))),
            app_owns_titlebar_drag: cfg!(target_os = "macos"),
            // macOS permits trackpad scrolling without focusing a window. Pace
            // that interaction normally; the demand-driven clock sleeps at idle.
            inactive_frame_interval: None,
            titlebar: Some(TitlebarOptions {
                title: Some(
                    config["productName"]
                        .as_str()
                        .expect("application name")
                        .to_owned()
                        .into(),
                ),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| PlayerApp::new(window, cx)),
    )
    .expect("failed to open SoundGrammy");
    cx.activate(true);
}
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn configure_menus(cx: &mut App) {
    use gpui_kit::component::input;
    let name = crate::runtime::application_config()["productName"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut app_items = vec![
        MenuItem::action(format!("About {name}"), About),
        MenuItem::separator(),
    ];
    if cfg!(target_os = "macos") {
        app_items.extend([
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(format!("Hide {name}"), HideApp),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
        ]);
    }
    app_items.extend([
        MenuItem::separator(),
        MenuItem::action(format!("Quit {name}"), Quit),
    ]);
    cx.set_menus(vec![
        Menu::new(name).items(app_items),
        Menu::new("File").items([
            MenuItem::action("Close Window", Quit),
            MenuItem::action("Close All", Quit),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Undo", input::Undo),
            MenuItem::action("Redo", input::Redo),
            MenuItem::separator(),
            MenuItem::action("Cut", input::Cut),
            MenuItem::action("Copy", input::Copy),
            MenuItem::action("Paste", input::Paste),
            MenuItem::action("Select All", input::SelectAll),
        ]),
        Menu::new("View").items([MenuItem::action("Toggle Full Screen", WindowFullscreen)]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
            MenuItem::separator(),
            MenuItem::action("Close Window", Quit),
        ]),
        Menu::new("Help").items(Vec::<MenuItem>::new()),
    ]);
}

fn mono_font() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(target_os = "windows") {
        "Consolas"
    } else {
        "monospace"
    }
}
pub(super) fn configure_theme(cx: &mut App) {
    gpui_kit::base::TooltipDefaults::new()
        .with_grace_period(Duration::ZERO)
        .install(cx);
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.colors.primary = rgb(0x2aabee).into();
        theme.colors.primary_foreground = rgb(0xfafafa).into();
        theme.colors.button_primary = rgb(0x2aabee).into();
        theme.colors.button_primary_foreground = rgb(0x111111).into();
        theme.colors.primary_hover = rgb(0x2398d5).into();
        theme.colors.primary_active = rgb(0x2086bd).into();
        theme.colors.button_primary_hover = rgb(0x2398d5).into();
        theme.colors.button_primary_active = rgb(0x2086bd).into();
        theme.colors.slider_bar = rgb(0x2aabee).into();
        theme.colors.slider_thumb = rgb(0x2aabee).into();
        theme.radius = px(12.);
        theme.radius_lg = px(16.);
        theme.colors.input = rgb(0x383838).into();
        theme.colors.popover = rgb(0x202020).into();
        theme.colors.popover_foreground = rgb(0xf2f2f2).into();
        theme.colors.background = rgb(0x141414).into();
        theme.colors.foreground = rgb(0xf2f2f2).into();
        theme.colors.secondary = rgb(0x272727).into();
        theme.colors.border = rgb(0x383838).into();
        theme.colors.muted_foreground = rgb(0xa5a5a5).into();
    });
}
pub(super) fn bind_shortcuts(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", Quit, None),
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("cmd-h", HideApp, None),
        KeyBinding::new("cmd-alt-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("ctrl-cmd-f", WindowFullscreen, None),
        KeyBinding::new(
            "space",
            TogglePlayback,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
        KeyBinding::new(
            "cmd-right",
            NextTrack,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
        KeyBinding::new(
            "ctrl-right",
            NextTrack,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
        KeyBinding::new(
            "cmd-left",
            PreviousTrack,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
        KeyBinding::new(
            "ctrl-left",
            PreviousTrack,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
        KeyBinding::new(
            "right",
            SeekForward,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
        KeyBinding::new(
            "left",
            SeekBackward,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
        KeyBinding::new("escape", Escape, Some("SoundGrammy")),
        KeyBinding::new(
            "cmd-,",
            OpenSettings,
            Some("SoundGrammy && !Input && !Button && !Slider && !Menu && !TrackRow && !PlaylistTabs && !PlaylistSort"),
        ),
    ]);
}
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Loading,
    Login,
    Ready,
}
#[derive(Clone, PartialEq)]
enum LoginStep {
    Qr,
    Phone,
    Code,
    Password { qr: bool, hint: Option<String> },
}
#[derive(Clone)]
enum Panel {
    Navigation,
    About,
    PlaylistFilters,
    Volume,
    Search,
    Settings,
    Proxy,
    Queue,
    NewPlaylist,
    RenamePlaylist(i64),
    SaveQueue,
    TrackInfo(i64),
    Logs,
    DownloadResult,
    CompactPlayer,
    Confirm(Confirmation),
    Import {
        path: String,
        preview: crate::playlist_recipe::PlaylistImportPreview,
    },
}
#[derive(Clone)]
enum Confirmation {
    Logout,
    FullReindex,
    ClearCache,
    ClearStatistics,
    DeletePlaylist(i64),
    DisconnectLastFm,
}
#[derive(Clone)]
enum TrackAction {
    Next,
    Append,
    Like,
    SetLiked(bool),
    Cache,
    RemoveCache,
    Export,
    AddToPlaylist(i64),
    Remove,
    Info,
}
struct Inputs {
    phone: Entity<InputState>,
    code: Entity<InputState>,
    password: Entity<InputState>,
    search: Entity<InputState>,
    playlist_search: Entity<InputState>,
    name: Entity<InputState>,
    proxy_link: Entity<TextareaState>,
    proxy_server: Entity<InputState>,
    proxy_port: Entity<InputState>,
    proxy_secret: Entity<InputState>,
    cache_limit: Entity<InputState>,
    cache_ttl: Entity<InputState>,
    exact_volume: Entity<InputState>,
}
struct PlayerApp {
    clear_track_search: bool,
    reset_inputs: bool,
    pending_full_sync: bool,
    panel_stack: Vec<Panel>,
    retained_modal: Option<Panel>,
    modal_progress: f32,
    bounce_ready_at: Option<std::time::Instant>,
    #[cfg(test)]
    inactive_row_renders: std::cell::Cell<usize>,
    track_equalizer: Entity<motion::Equalizer>,
    login_equalizer: Entity<motion::Equalizer>,
    wake_requests: Option<tokio::sync::mpsc::UnboundedSender<bool>>,
    seek_preview: Option<(f32, f64)>,
    seek_pointer: Option<Point<Pixels>>,
    pending_back: bool,
    settings_sections: std::collections::HashSet<&'static str>,
    download_results: std::collections::VecDeque<crate::export::PlaylistDownloadResult>,
    download_sections: std::collections::HashSet<&'static str>,
    login_focus_step: Option<&'static str>,
    queue_save_scope: QueueSaveScope,
    queue_scope_focus: [FocusHandle; 3],
    playlist_tab_focus: [FocusHandle; 2],
    playlist_sort_focus: [FocusHandle; 3],
    loading_started: Option<std::time::Instant>,
    loading_indicator_task: Option<Task<()>>,
    new_playlist_import: bool,
    new_playlist_tracks: Vec<i64>,
    touch_press: Option<(usize, Point<Pixels>, u64)>,
    touch_generation: u64,
    suppress_touch_click: Option<usize>,
    queue_list: ListState,
    queue_items: Vec<QueueItem>,
    hq_thumbs: std::collections::HashSet<i64>,
    thumb_loading: std::collections::HashSet<i64>,
    metadata: Option<(i64, commands::TrackMetadata)>,
    volume_wheel: f64,
    volume_hover: bool,
    volume_pointer_over: [bool; 2],
    volume_dragging: bool,
    volume_close_task: Option<Task<()>>,
    volume_exact_open: bool,
    volume_focus: FocusHandle,
    runtime: Option<AppRuntime>,
    host: Option<AppHost>,
    phase: Phase,
    epoch: u64,
    hover_title_epoch: std::rc::Rc<std::cell::Cell<u64>>,
    user: Option<AuthUser>,
    library: Library,
    preferences: Preferences,
    snapshot: audio::Snapshot,
    login: LoginStep,
    qr: Option<(usize, Vec<bool>)>,
    qr_request: u64,
    inputs: Inputs,
    focus: FocusHandle,
    modal_focus: FocusHandle,
    panel_was_open: bool,
    panel_return_focus: Option<FocusHandle>,
    seek_slider: Entity<SliderState>,
    synced_seek_percent: Option<f32>,
    synced_volume_percent: Option<f32>,
    volume_slider: Entity<SliderState>,
    subscriptions: Vec<Subscription>,
    panel: Option<Panel>,
    busy: usize,
    error: Option<String>,
    notice: Option<String>,
    syncing: bool,
    sync_progress: Option<(i64, i64)>,
    online: bool,
    compact_playlist: bool,
    fullscreen: bool,
    fullscreen_request: u64,
    fullscreen_activity: std::time::Instant,
    fullscreen_idle_task: Option<Task<()>>,
    fullscreen_cursor_hidden: bool,
    fullscreen_visual: Entity<artwork::FullscreenVisual>,
    artwork_cache: Entity<artwork::ThumbnailCache>,
    fullscreen_pointer: Option<Point<Pixels>>,
    selection_mode: bool,
    thumbs: HashMap<i64, Option<std::path::PathBuf>>,
    download_progress: HashMap<i64, f64>,
    avatar: Option<std::path::PathBuf>,
    jobs: HashMap<String, String>,
    logs: diagnostics::LogBook,
    proxy: crate::proxy_settings::ProxySettings,
    proxy_status: Option<crate::proxy_settings::ProxySettingsView>,
    cache_settings: Option<crate::cache::CacheSettings>,
    cache_usage: Option<crate::cache::CacheUsage>,
    lastfm: Option<crate::lastfm::models::LastFmStatus>,
    statistics_enabled: bool,
    mutations: Arc<tokio::sync::Mutex<()>>,
    command_port: Option<transport::CommandPort>,
    reconnect_running: bool,
    bounce_profile: Option<Arc<artwork::BounceProfile>>,
    bounce_track: Option<i64>,
    drag_track: Option<usize>,
    sort_drag: Option<views::SortDrag>,
    scrubbing: Option<f64>,
    playlist_search: String,
    previous_volume: f64,
    palette: artwork::Palette,
    playing_artwork_path: Option<std::path::PathBuf>,
    playing_artwork: Option<Arc<RenderImage>>,
    palette_path: Option<std::path::PathBuf>,
    preparing_artwork_path: Option<std::path::PathBuf>,
    prepared_artwork_track: Option<i64>,
    fullscreen_cover: Option<Arc<RenderImage>>,
    pending_fullscreen: bool,
    #[cfg(target_os = "macos")]
    blur_surface: Option<Arc<gpui_apple::effects::Surface>>,
    fullscreen_backdrop: artwork::Background,
}
impl PlayerApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_inner(
            window,
            cx,
            std::env::args().any(|arg| arg == "--preview" || arg == "--preview-fixture"),
        )
    }
    fn new_inner(window: &mut Window, cx: &mut Context<Self>, preview: bool) -> Self {
        let input = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let inputs = Inputs {
            phone: input("+447700900123", window, cx),
            code: input("Telegram login code", window, cx),
            password: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("2FA password")
                    .masked(true)
            }),
            search: input("Search tracks", window, cx),
            playlist_search: input("Search playlists", window, cx),
            name: input("My playlist", window, cx),
            proxy_link: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .rows(3)
                    .placeholder("tg://proxy?server=127.0.0.1&port=1443&secret=…")
            }),
            proxy_server: input("Server", window, cx),
            proxy_port: input("Port", window, cx),
            proxy_secret: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Secret")
                    .masked(true)
            }),
            cache_limit: input("Size limit in GB", window, cx),
            exact_volume: input("0–100", window, cx),
            cache_ttl: input("Keep for days", window, cx),
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
        let mut app = Self {
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
            login_equalizer: cx.new(|_| motion::Equalizer::new(true)),
            wake_requests: None,
            seek_preview: None,
            seek_pointer: None,
            pending_back: false,
            settings_sections: Default::default(),
            download_results: Default::default(),
            download_sections: Default::default(),
            login_focus_step: None,
            queue_save_scope: QueueSaveScope::Full,
            queue_scope_focus: std::array::from_fn(|_| cx.focus_handle()),
            playlist_tab_focus: std::array::from_fn(|_| cx.focus_handle()),
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
            hover_title_epoch: Default::default(),
            user: None,
            library: Default::default(),
            preferences: Default::default(),
            snapshot: Default::default(),
            login: LoginStep::Qr,
            qr: None,
            qr_request: 0,
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
            fullscreen_visual: cx.new(|_| artwork::FullscreenVisual::new(artwork_cache.clone())),
            artwork_cache,
            fullscreen_pointer: None,
            selection_mode: false,
            thumbs: HashMap::new(),
            download_progress: Default::default(),
            avatar: None,
            jobs: HashMap::new(),
            logs: Default::default(),
            proxy: Default::default(),
            proxy_status: None,
            cache_settings: None,
            cache_usage: None,
            lastfm: None,
            statistics_enabled: true,
            mutations: Default::default(),
            command_port: None,
            reconnect_running: false,
            bounce_profile: None,
            bounce_track: None,
            drag_track: None,
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
            #[cfg(target_os = "macos")]
            blur_surface: gpui_apple::effects::Surface::new(
                gpui_apple::effects::Effect::BackdropBlur {
                    radius: 16.,
                    opacity: 1.,
                },
            )
            .map(Arc::new),
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
                    app.library.search = input.read(cx).value().to_string();
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
                                value.start() as f64 / 100. * app.snapshot.duration_seconds,
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
        app.subscriptions.push(cx.subscribe_in(
            &app.inputs.code,
            window,
            |app, input, event, window, cx| {
                if matches!(event, InputEvent::Change)
                    && app.phase == Phase::Login
                    && matches!(app.login, LoginStep::Code)
                {
                    app.error = None;
                    cx.notify();
                    let value = input.read(cx).value();
                    if app.busy == 0
                        && value.len() == 5
                        && value.chars().all(|v| v.is_ascii_digit())
                    {
                        app.submit_login(window, cx);
                    }
                }
            },
        ));
        for state in [
            app.inputs.phone.clone(),
            app.inputs.code.clone(),
            app.inputs.password.clone(),
        ] {
            app.subscriptions.push(
                cx.subscribe_in(&state, window, |app, _, event, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        app.submit_login(window, cx);
                    }
                }),
            );
        }
        match AppRuntime::open_with_preview(preview) {
            Ok((mut runtime, mut messages)) => {
                let host = runtime.host();
                #[cfg(target_os = "windows")]
                if let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
                    if let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() {
                        host.set_native_window_handle(handle.hwnd.get() as usize);
                    }
                }
                if !preview {
                    runtime.start();
                }
                app.preferences = Preferences::load(&host);
                app.logs = diagnostics::LogBook::load(host.clone(), app.preferences.logs_enabled);
                app.library.selected = PlaylistId::parse(&app.preferences.selected);
                app.statistics_enabled = host
                    .state::<AppState>()
                    .db
                    .get_setting(crate::db::SETTING_LISTEN_STATS_ENABLED)
                    .ok()
                    .flatten()
                    .as_deref()
                    != Some("false");
                if !app.statistics_enabled
                    && matches!(&app.library.selected, PlaylistId::Builtin(id) if id=="popular" || id=="recent")
                {
                    app.library.select(PlaylistId::All);
                    app.preferences.selected = "all".into();
                }
                app.host = Some(host);
                app.runtime = Some(runtime);
                cx.spawn(async move |view, cx| {
                    while let Some(message) = messages.recv().await {
                        match message {
                            UiMessage::MainThread(work) => {
                                if view.update(cx, |_, _| ()).is_err() {
                                    break;
                                }
                                work();
                            }
                            UiMessage::Event(event) => {
                                if view.update(cx, |app, cx| app.event(event, cx)).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                })
                .detach();
                if preview {
                    app.set_preview();
                } else {
                    app.bootstrap(cx);
                }
            }
            Err(error) => {
                app.phase = Phase::Login;
                app.error = Some(error.to_string());
            }
        }
        if preview && std::env::args().any(|arg| arg == "--preview-compact") {
            window.resize(size(px(390.), px(844.)));
        }
        app.focus.focus(window, cx);
        if preview {
            app.start_preview_performance(window, cx);
        }
        app
    }
    fn host(&self) -> AppHost {
        self.host.clone().expect("application runtime unavailable")
    }
    fn save_preferences(&mut self) {
        self.logs.set_enabled(self.preferences.logs_enabled);
        if let Some(host) = &self.host {
            if let Err(e) = self.preferences.save(host) {
                self.logs.push(e.to_string());
            }
        }
    }
    fn task<T, F, E>(
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
    fn quiet_task<T, F, E>(
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
    fn task_inner<T, F, E>(
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
        cx.spawn(async move |view, cx| {
            let result = job
                .await
                .map_err(|_| "A background operation failed.".to_owned())
                .and_then(|v| v);
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
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn bootstrap(&mut self, cx: &mut Context<Self>) {
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
    fn authenticated(&mut self, user: AuthUser, cx: &mut Context<Self>) {
        self.user = Some(user);
        self.phase = Phase::Loading;
        self.qr = None;
        self.qr_request += 1;
        self.hydrate(cx, true);
        let host = self.host();
        self.quiet_task(
            cx,
            async move { commands::get_user_avatar(&host.state::<AppState>()).await },
            |app, path, _| app.avatar = path.map(Into::into),
        );
    }
    fn hydrate(&mut self, cx: &mut Context<Self>, initial: bool) {
        let host = self.host();
        self.task(cx, model::hydrate(host), move |app, mut library, cx| {
            library.selected = app.library.selected.clone();
            if let PlaylistId::Custom(id) = library.selected {
                if !library
                    .playlists
                    .as_ref()
                    .is_some_and(|p| p.custom.iter().any(|p| p.id == id))
                {
                    library.selected = PlaylistId::All;
                }
            }
            if library.selected == app.library.selected
                && library.membership() == app.library.membership()
            {
                library.selection = app.library.selection.clone();
                library.selection_anchor = app.library.selection_anchor;
            }
            library.search = app.library.search.clone();
            library.sort = app.library.sort.clone();
            library.descending = app.library.descending;
            library.additional_sort = app.library.additional_sort.clone();
            app.library = library;
            app.phase = Phase::Ready;
            if initial {
                let preferences = audio::session::Preferences {
                    repeat: app.preferences.repeat,
                    shuffle: app.preferences.shuffle,
                    mode: app.preferences.shuffle_mode,
                };
                app.command(
                    Command::Attach {
                        preferences: Some(preferences),
                    },
                    cx,
                );
                app.transport(audio::Control::Volume(app.preferences.volume), cx);
                app.reconnect(cx);
            }
        });
    }
    fn reconnect(&mut self, cx: &mut Context<Self>) {
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
                                app.reset();
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
    fn sync(&mut self, full: bool, cx: &mut Context<Self>) {
        if self.syncing {
            self.pending_full_sync |= full;
            return;
        }
        self.syncing = true;
        self.sync_progress = None;
        let host = self.host();
        self.task(
            cx,
            async move {
                let state = host.state::<AppState>();
                Ok::<_, std::convert::Infallible>(
                    commands::sync_saved_music(
                        &state,
                        host,
                        Some(if full {
                            crate::telegram::saved_music::SyncMode::Full
                        } else {
                            crate::telegram::saved_music::SyncMode::Soft
                        }),
                    )
                    .await,
                )
            },
            |app, result, cx| {
                app.syncing = false;
                app.sync_progress = None;
                if let Err(error) = result {
                    app.logs.push(error.to_string());
                    app.error = Some(error.to_string());
                }
                app.hydrate(cx, false);
                if std::mem::take(&mut app.pending_full_sync) {
                    app.sync(true, cx);
                }
            },
        );
    }
    fn start_qr(&mut self, restart: bool, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        self.login = LoginStep::Qr;
        self.qr = None;
        self.qr_request += 1;
        self.poll_qr(restart, true, self.qr_request, cx);
    }
    fn poll_qr(&mut self, restart: bool, initial: bool, request: u64, cx: &mut Context<Self>) {
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
                if app.qr_request != request || app.login != LoginStep::Qr {
                    return;
                }
                match outcome {
                    QrOutcome::Authorized { user } => app.authenticated(user, cx),
                    QrOutcome::PasswordRequired { hint } => {
                        app.login = LoginStep::Password { qr: true, hint };
                        app.qr = None;
                    }
                    QrOutcome::Waiting { url, .. } => {
                        if let Ok(code) = qrcode::QrCode::new(url.as_bytes()) {
                            app.qr = Some((
                                code.width(),
                                code.to_colors()
                                    .into_iter()
                                    .map(|v| v == qrcode::Color::Dark)
                                    .collect(),
                            ));
                        }
                        cx.spawn(async move |view, cx| {
                            crate::runtime::spawn(async {
                                tokio::time::sleep(Duration::from_secs(2)).await
                            })
                            .await
                            .ok();
                            let _ = view.update(cx, |app, cx| {
                                if app.qr_request == request && app.login == LoginStep::Qr {
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
    fn submit_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy > 0 || self.host.is_none() {
            return;
        }
        let host = self.host();
        match self.login.clone() {
            LoginStep::Phone => {
                let phone = match login_phone(self.inputs.phone.read(cx).value().as_ref()) {
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
                        PhoneSendCodeOutcome::CodeSent => app.login = LoginStep::Code,
                        PhoneSendCodeOutcome::Authorized { user } => app.authenticated(user, cx),
                    },
                );
            }
            LoginStep::Code => {
                let code = match login_code(self.inputs.code.read(cx).value().as_ref()) {
                    Ok(value) => value,
                    Err(error) => {
                        self.error = Some(error.to_string());
                        return;
                    }
                };
                self.task(cx,async move{commands::auth::phone_sign_in(&host.state::<AppState>(),code).await},|app,result,cx|match result{AuthOutcome::Authorized{user}=>app.authenticated(user,cx),AuthOutcome::PasswordRequired{hint}=>app.login=LoginStep::Password{qr:false,hint}});
            }
            LoginStep::Password { qr, .. } => {
                let password = self.inputs.password.read(cx).value().to_string();
                self.inputs
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
    fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        self.transport(command.into(), cx);
    }
    fn transport(&mut self, control: audio::Control, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        if self.command_port.is_none() {
            self.command_port = Some(transport::CommandPort::new(self.host()));
        }
        let response = self.command_port.as_ref().unwrap().send(control);
        self.task(
            cx,
            async move {
                response
                    .await
                    .map_err(|_| "Playback operation cancelled".to_owned())?
            },
            |app, snapshot, _| {
                if snapshot.revision >= app.snapshot.revision {
                    app.accept_snapshot(snapshot);
                }
            },
        );
    }
    fn seek(&mut self, seconds: f64, cx: &mut Context<Self>) {
        if !seconds.is_finite() || !self.snapshot.duration_seconds.is_finite() {
            return;
        }
        self.transport(
            audio::Control::SeekAttempt(
                seconds.clamp(0., self.snapshot.duration_seconds.max(0.)),
                self.snapshot.attempt_id.clone(),
            ),
            cx,
        );
    }
    fn play_entries(
        &mut self,
        start: usize,
        shuffle: Option<audio::session::Shuffle>,
        cx: &mut Context<Self>,
    ) {
        let entries = self.library.entries();
        if entries.is_empty() {
            return;
        }
        let tracks = entries.iter().map(|e| e.track.clone()).collect::<Vec<_>>();
        let source_indices = Some(entries.iter().map(|e| e.source_index).collect());
        let queue = Queue {
            source: Some(Source {
                r#type: SourceType::Playlist,
                playlist_id: match &self.library.selected {
                    PlaylistId::Builtin(id) if id == "liked" => self
                        .library
                        .playlists
                        .as_ref()
                        .map(|p| serde_json::json!(p.liked.id))
                        .unwrap_or_else(|| self.library.selected.value()),
                    _ => self.library.selected.value(),
                },
                name: self.library.name(),
                track_ids: self.library.membership(),
            }),
            tracks,
            cursor: start.min(entries.len() - 1) as i64,
            source_indices,
            base_entries: None,
        };
        self.command(
            Command::PlayPlaylist {
                queue,
                shuffle,
                toggle_if_current: true,
                pin_start: start > 0 || shuffle.is_none(),
            },
            cx,
        );
    }
    fn set_statistics_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.statistics_enabled = enabled;
        if !enabled
            && matches!(&self.library.selected, PlaylistId::Builtin(id) if id=="popular" || id=="recent")
        {
            self.select_playlist(PlaylistId::All, cx);
        }
    }
    fn select_playlist(&mut self, id: PlaylistId, cx: &mut Context<Self>) {
        if id != self.library.selected {
            self.clear_track_search = true;
        }
        self.library.select(id);
        self.preferences.selected = self.library.selected.key();
        self.compact_playlist = true;
        self.save_preferences();
        cx.notify();
    }
    // Progress notifications deliberately omit the unchanged queue/session.
    // Merge them with the last full snapshot instead of erasing presentation.
    fn accept_snapshot(&mut self, mut snapshot: audio::Snapshot) {
        if snapshot.player.is_none() {
            snapshot.player = self.snapshot.player.clone();
        }
        if snapshot
            .player
            .as_ref()
            .is_some_and(|p| p.queue.tracks.is_empty())
        {
            self.panel_stack
                .retain(|p| !matches!(p, Panel::Queue | Panel::CompactPlayer));
            if matches!(self.panel, Some(Panel::Queue | Panel::CompactPlayer)) {
                self.panel = self.panel_stack.pop();
            }
        }
        self.snapshot = snapshot;
        self.synced_seek_percent = None;
    }
    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        if self.user.is_none() && !matches!(event.name, "auth:revoked" | "ui:back") {
            return;
        }
        match event.name {
            "ui:back" => self.pending_back = true,
            "audio:state" => {
                let previous_preferences = (
                    self.preferences.repeat,
                    self.preferences.shuffle,
                    self.preferences.shuffle_mode,
                );
                if let Some(snapshot) = event.payload::<audio::Snapshot>() {
                    let Some(user) = self.user.as_ref() else {
                        return;
                    };
                    if snapshot.player.as_ref().is_some_and(|p| {
                        p.queue
                            .tracks
                            .iter()
                            .any(|t| t.tg_user_id != 0 && t.tg_user_id != user.id)
                    }) {
                        return;
                    }
                    if snapshot.revision < self.snapshot.revision {
                        return;
                    }
                    // Paused output may publish heartbeat revisions without any
                    // visible change. Keep ordering without waking every view.
                    let previous = &self.snapshot;
                    if snapshot.player.is_none()
                        && snapshot.error.is_none()
                        && previous.error.is_none()
                        && snapshot.last_control == previous.last_control
                        && snapshot.kind == previous.kind
                        && snapshot.status == previous.status
                        && snapshot.track_id == previous.track_id
                        && snapshot.attempt_id == previous.attempt_id
                        && snapshot.current_time_seconds == previous.current_time_seconds
                        && snapshot.duration_seconds == previous.duration_seconds
                        && snapshot.buffered_ranges == previous.buffered_ranges
                        && snapshot.volume_percent == previous.volume_percent
                        && snapshot.initial_loading == previous.initial_loading
                        && snapshot.seeking == previous.seeking
                    {
                        self.snapshot.revision = snapshot.revision;
                        return;
                    }
                    if snapshot.revision >= self.snapshot.revision {
                        if let Some(error) = &snapshot.error {
                            if self
                                .snapshot
                                .error
                                .as_ref()
                                .is_none_or(|old| old.message != error.message)
                            {
                                self.logs.push(error.message.clone());
                                self.error = Some(error.message.clone());
                            }
                        }
                        self.accept_snapshot(snapshot.clone());
                        self.preferences.repeat = self
                            .snapshot
                            .player
                            .as_ref()
                            .map(|p| p.preferences.repeat)
                            .unwrap_or_default();
                        self.preferences.shuffle = self
                            .snapshot
                            .player
                            .as_ref()
                            .map(|p| p.preferences.shuffle)
                            .unwrap_or_default();
                        self.preferences.shuffle_mode = self
                            .snapshot
                            .player
                            .as_ref()
                            .map(|p| p.preferences.mode)
                            .unwrap_or_default();
                        if previous_preferences
                            != (
                                self.preferences.repeat,
                                self.preferences.shuffle,
                                self.preferences.shuffle_mode,
                            )
                        {
                            self.save_preferences();
                        }
                    }
                }
            }
            "auth:revoked" => self.reset(),
            "library:batch" => {
                if let Some(batch) = event.payload::<crate::db::LibraryBatch>() {
                    if self
                        .user
                        .as_ref()
                        .is_some_and(|u| u.id == batch.status.tg_user_id)
                        && self.library.apply_batch(batch)
                    {
                        for id in &batch.removed_track_ids {
                            self.thumbs.remove(id);
                            self.download_progress.remove(id);
                            self.hq_thumbs.remove(id);
                        }
                    }
                }
            }
            "sync:start" => self.syncing = true,
            "sync:done" => {
                self.syncing = false;
                self.sync_progress = None;
            }
            "sync:error" => {
                self.syncing = false;
                self.sync_progress = None;
                if let Some(e) = event.payload::<crate::telegram::saved_music::SyncError>() {
                    self.error = Some(e.message.clone());
                }
            }
            "sync:progress" => {
                if let Some(progress) =
                    event.payload::<crate::telegram::saved_music::SyncProgress>()
                {
                    self.sync_progress = Some((progress.done as i64, progress.total));
                }
            }
            "download:progress" => {
                if let Some(progress) = event.payload::<crate::streaming::DownloadProgress>() {
                    let id = progress.track_id();
                    if self.library.tracks.iter().any(|t| t.id == id) {
                        if let Some(ratio) = progress.ratio() {
                            self.download_progress.insert(id, ratio);
                        } else {
                            self.download_progress.remove(&id);
                        }
                    }
                }
            }
            "cache:changed" => {
                if let Some(changed) = event.payload::<crate::cache::CacheChanged>() {
                    if changed.cleared {
                        self.library.cached.clear();
                        self.download_progress.clear();
                    }
                    for id in &changed.track_ids {
                        self.download_progress.remove(id);
                        if changed.cached {
                            self.library.cached.insert(*id);
                        } else {
                            self.library.cached.remove(id);
                        }
                    }
                }
            }
            "listen:stats" => {
                if let Some(stats) = event.payload::<crate::db::TrackListenStats>() {
                    self.library.stats.insert(stats.track_id, stats.clone());
                    if matches!(&self.library.selected, PlaylistId::Builtin(id) if id == "popular" || id == "recent")
                    {
                        self.library.invalidate_track_list();
                    }
                }
            }
            "lastfm:status_changed" => {
                if let Some(status) = event.payload::<crate::lastfm::models::LastFmStatus>() {
                    self.lastfm = Some(status.clone());
                }
            }
            "artwork:changed" => {
                if let Some(art) = event.payload::<crate::cache::media_art::ArtworkChanged>() {
                    if self.user.as_ref().is_some_and(|u| u.id == art.tg_user_id) {
                        self.accept_thumbnail(art.track_id, art.path.clone().into());
                    }
                }
            }
            "cache_tracks:progress" => {
                if let Some(job) = event.payload::<crate::cache::CacheTracksProgress>() {
                    if !self.jobs.contains_key(&job.job_id) {
                        return;
                    }
                    self.jobs.insert(
                        job.job_id.clone(),
                        format!("Caching: {} / {}", job.current, job.total),
                    );
                }
            }
            "download_playlist:progress" => {
                if let Some(job) = event.payload::<crate::export::PlaylistDownloadProgress>() {
                    if !self.jobs.contains_key(&job.job_id) {
                        return;
                    }
                    self.jobs.insert(
                        job.job_id.clone(),
                        format!("Downloading: {} / {}", job.current, job.total),
                    );
                }
            }
            _ => {}
        }
        cx.notify();
    }
    fn reset(&mut self) {
        self.dismiss_hover_titles();
        self.reset_inputs = true;
        self.epoch += 1;
        self.qr_request += 1;
        self.user = None;
        self.phase = Phase::Login;
        self.library = Default::default();
        self.snapshot = Default::default();
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
        self.login_focus_step = None;
        self.lastfm = None;
        self.online = false;
        self.syncing = false;
        self.pending_full_sync = false;
        self.reconnect_running = false;
        self.compact_playlist = false;
        self.selection_mode = false;
        self.qr = None;
        self.login = LoginStep::Phone;
        self.bounce_track = None;
        self.bounce_profile = None;
        self.mutations = Default::default();
        self.command_port = None;
    }
    fn fullscreen_controls_visible(&self, cx: &App) -> bool {
        cx.background_executor()
            .now()
            .saturating_duration_since(self.fullscreen_activity)
            < Duration::from_secs(3)
            || self.panel.is_some()
            || self.volume_hover
            || self.scrubbing.is_some()
    }
    fn sync_fullscreen_cursor(&mut self, window: &Window, cx: &App) {
        let hidden = self.fullscreen
            && self.phase == Phase::Ready
            && window.is_window_active()
            && window.is_visible()
            && !self.fullscreen_controls_visible(cx);
        let restore =
            self.fullscreen && !hidden && window.is_window_active() && !cx.is_cursor_visible();
        if self.fullscreen_cursor_hidden != hidden || (hidden && cx.is_cursor_visible()) || restore
        {
            self.fullscreen_cursor_hidden = hidden;
            set_fullscreen_cursor_hidden(window, hidden);
        }
    }
    fn show_fullscreen_controls(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.fullscreen_activity = cx.background_executor().now();
        self.fullscreen_idle_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = view.update(cx, |_, cx| cx.notify());
        }));
        cx.notify();
    }
    fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_hover_titles();
        self.reset_volume_hover();
        if self.pending_fullscreen {
            self.pending_fullscreen = false;
            return;
        }
        if !self.fullscreen && self.fullscreen_artwork_pending() {
            self.pending_fullscreen = true;
            cx.notify();
            return;
        }
        self.fullscreen = !self.fullscreen;
        if self.fullscreen {
            // Reopening fullscreen retries a failed/deferred embedded-art probe.
            if let Some(id) = self.snapshot.track_id {
                self.hq_thumbs.remove(&id);
            }
            self.show_fullscreen_controls(window, cx);
        } else {
            self.fullscreen_idle_task = None;
            self.fullscreen_visual
                .update(cx, |visual, _| visual.clear());
        }
        self.fullscreen_pointer = None;
        // AppKit snapshots the current window to animate its Space transition.
        // Let the new player/library scene paint first; otherwise that snapshot
        // can briefly restore the old scene after the fullscreen scene appears.
        self.fullscreen_request = self.fullscreen_request.wrapping_add(1);
        let request = self.fullscreen_request;
        let epoch = self.epoch;
        let target = self.fullscreen;
        let view = cx.weak_entity();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let _ = view.update(cx, |app, _| {
                    if app.epoch == epoch
                        && app.fullscreen_request == request
                        && window.is_fullscreen() != target
                    {
                        window.toggle_fullscreen();
                    }
                });
            });
        });
        self.focus.focus(window, cx);
        self.apply_display_awake(cx);
    }
    fn apply_display_awake(&mut self, _cx: &mut Context<Self>) {
        if self.wake_requests.is_none() {
            let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<bool>();
            let host = self.host();
            crate::runtime::spawn(async move {
                while let Some(enabled) = receiver.recv().await {
                    let host = host.clone();
                    let result = crate::runtime::spawn_blocking(move || {
                        host.try_state::<crate::display_wake::DisplayWakeState>()
                            .map(|state| state.set_enabled(enabled))
                    })
                    .await;
                    if let Ok(Some(Err(error))) = result {
                        tracing::warn!(%error,"Display wake request failed");
                    }
                }
            });
            self.wake_requests = Some(sender);
        }
        let _ = self
            .wake_requests
            .as_ref()
            .unwrap()
            .send(self.fullscreen && self.preferences.keep_awake);
    }
    fn dismiss_hover_titles(&self) {
        self.hover_title_epoch
            .set(self.hover_title_epoch.get().wrapping_add(1));
    }
    fn show_panel(&mut self, panel: Panel) {
        self.dismiss_hover_titles();
        if !matches!(
            panel,
            Panel::Queue | Panel::PlaylistFilters | Panel::CompactPlayer | Panel::Search
        ) {
            self.reset_volume_hover();
            self.seek_preview = None;
            self.seek_pointer = None;
        }
        if let Some(parent) = self.panel.take() {
            self.panel_stack.push(parent);
        }
        self.panel = Some(panel);
    }
    fn close_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_hover_titles();
        if matches!(self.panel, Some(Panel::DownloadResult)) {
            self.download_results.pop_front();
            self.download_sections.clear();
            if !self.download_results.is_empty() {
                cx.notify();
                return;
            }
        }
        if matches!(self.panel, Some(Panel::Search)) {
            self.library.search.clear();
            self.inputs
                .search
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.panel = self.panel_stack.pop();
        cx.notify();
    }
    fn open_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        self.touch_press = None;
        if matches!(panel, Panel::Settings) {
            self.settings_sections.clear();
        }
        if matches!(panel, Panel::CompactPlayer) {
            if let Some(id) = self.snapshot.track_id {
                self.hq_thumbs.remove(&id);
            }
        }
        if matches!(panel, Panel::SaveQueue) {
            self.queue_save_scope = QueueSaveScope::Full;
        }
        if matches!(panel, Panel::NewPlaylist) {
            self.new_playlist_tracks.clear();
            self.new_playlist_import = false;
        }
        if matches!(panel, Panel::Volume) {
            self.inputs.exact_volume.update(cx, |s, cx| {
                s.set_value(self.preferences.volume.round().to_string(), window, cx)
            });
        }
        self.error = None;
        match panel {
            Panel::NewPlaylist | Panel::SaveQueue => self
                .inputs
                .name
                .update(cx, |s, cx| s.set_value("", window, cx)),
            Panel::RenamePlaylist(id) => {
                let name = self
                    .library
                    .playlists
                    .as_ref()
                    .and_then(|p| p.custom.iter().find(|p| p.id == id))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                self.inputs
                    .name
                    .update(cx, |s, cx| s.set_value(name, window, cx));
            }
            Panel::Settings | Panel::Proxy => self.load_settings(window, cx),
            _ => {}
        }
        let name_focus = matches!(
            panel,
            Panel::NewPlaylist | Panel::SaveQueue | Panel::RenamePlaylist(_)
        );
        self.show_panel(panel);
        if name_focus && self.panel_was_open {
            self.inputs.name.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }
    fn load_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        let host = self.host();
        let proxy = crate::proxy_settings::load(&host.state::<AppState>().db).unwrap_or_default();
        self.proxy = proxy.clone();
        self.inputs.proxy_link.update(cx, |s, cx| {
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
        self.inputs
            .proxy_server
            .update(cx, |s, cx| s.set_value(proxy.server, window, cx));
        self.inputs
            .proxy_port
            .update(cx, |s, cx| s.set_value(proxy.port.to_string(), window, cx));
        self.inputs
            .proxy_secret
            .update(cx, |s, cx| s.set_value(proxy.secret, window, cx));
        if let Ok(settings) = crate::cache::get_cache_settings(&host.state::<AppState>()) {
            self.inputs.cache_limit.update(cx, |s, cx| {
                s.set_value(
                    format!("{}", settings.limit_bytes as f64 / 1024_f64.powi(3)),
                    window,
                    cx,
                )
            });
            self.inputs.cache_ttl.update(cx, |s, cx| {
                s.set_value(format!("{}", settings.ttl_secs / 86400), window, cx)
            });
            self.cache_settings = Some(settings);
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
                app.proxy_status = Some(proxy);
                app.cache_usage = Some(usage);
                app.set_statistics_enabled(enabled, cx);
                app.lastfm = Some(lastfm);
            },
        );
    }
    fn refresh_playlists(&mut self, cx: &mut Context<Self>) {
        let host = self.host();
        self.task(
            cx,
            async move { commands::playlists::list_playlists(&host.state::<AppState>()).await },
            |app, bundle, _| {
                let old = app.library.membership();
                app.library.playlists = Some(bundle);
                app.library.invalidate_track_list();
                if old != app.library.membership() {
                    app.library.selection.clear();
                }
            },
        );
    }
    fn thumbnail(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.thumbs.contains_key(&id) {
            return;
        }
        self.thumbs.insert(id, None);
        self.thumb_loading.insert(id);
        let host = self.host();
        self.quiet_task(
            cx,
            async move {
                let path = commands::get_track_thumbnail(
                    &host.state::<AppState>(),
                    host.clone(),
                    id,
                    Some(false),
                )
                .await
                .ok()
                .flatten();
                Ok::<_, crate::error::AppError>(path)
            },
            move |app, path, _| {
                app.thumb_loading.remove(&id);
                if app.thumbs.get(&id).is_none_or(Option::is_none) {
                    app.thumbs.insert(id, path.map(Into::into));
                }
            },
        );
    }
    fn accept_thumbnail(&mut self, id: i64, path: std::path::PathBuf) {
        // The ID3 notification and Telegram request race. Quality can only rise.
        let current = self.thumbs.get(&id).and_then(Option::as_deref);
        if artwork::prefer_thumbnail(current, &path) {
            self.thumbs.insert(id, Some(path));
        }
    }
    fn high_quality_thumbnail(&mut self, id: i64, cx: &mut Context<Self>) {
        if !self.hq_thumbs.insert(id) {
            return;
        }
        let host = self.host();
        self.quiet_task(
            cx,
            async move {
                commands::get_track_thumbnail(
                    &host.state::<AppState>(),
                    host.clone(),
                    id,
                    Some(true),
                )
                .await
            },
            move |app, path, _| {
                // The normal thumbnail can finish while the high-quality
                // request is in flight. It is a fallback, never the winner.
                if let Some(path) = path {
                    app.accept_thumbnail(id, path.into());
                }
            },
        );
    }
}

#[cfg(target_os = "macos")]
fn set_fullscreen_cursor_hidden(window: &Window, hidden: bool) {
    use objc2::{msg_send, runtime::AnyObject, runtime::Bool, MainThreadMarker};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Some(_) = MainThreadMarker::new() else {
        return;
    };
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: GPUI supplies a live GPUIView on the main thread. The local
    // backend selector updates AppKit and GPUI's cursor visibility together.
    unsafe {
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        let _: () = msg_send![view, gpuiSetCursorHiddenUntilMouseMoves: Bool::new(hidden)];
    }
}

#[cfg(not(target_os = "macos"))]
fn set_fullscreen_cursor_hidden(_: &Window, _: bool) {}

#[cfg(target_os = "macos")]
fn set_application_icon() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(include_bytes!("../../icons/icon.png"));
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // The marker guarantees AppKit access is on the main thread; NSImage
        // contains bundled PNG data and is retained by NSApplication.
        unsafe {
            NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image));
        }
    }
}
