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
mod startup;
#[cfg(test)]
mod tests;
mod theme;
mod transport;
use theme::*;
mod account;
mod app;
mod effects;
mod events;
mod fullscreen;
mod library;
mod panels;
mod player;
mod tasks;
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
use app::PlayerApp;
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

fn is_platform_event(event: &Event) -> bool {
    #[cfg(target_os = "android")]
    {
        matches!(event, Event::Back)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = event;
        false
    }
}
