//! Deterministic, account-free fixtures for visual comparison and UI tests.
use super::*;
use gpui_kit::InputEvent as _;
impl PlayerApp {
    pub(super) fn set_preview(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Ready;
        self.library.clone().update(cx, |library, cx| {
            *library = Default::default();
            cx.notify();
        });
        self.user = Some(AuthUser {
            id: 1,
            first_name: "SoundGrammy".into(),
            last_name: None,
            username: None,
            phone: None,
        });
        self.library.clone().update(cx, |library, cx| { library.tracks=[("Synthetic","SpaceYeti",196),("Rabbit Hole","Dr. Ozi",206),("Throw Sum Mo’","YDG & Flozone",198),("Can We Call It Something Besides Brostep?","Killed It!",231),("Godhead Interface","Zardonic",228),("Damage","Figure",215),("KABOOM","Space Laces",198)].into_iter().enumerate().map(|(i,(title,artist,duration))|serde_json::from_value(serde_json::json!({"id":i as i64+1,"title":title,"performer":artist,"duration":duration,"mime_type":"audio/mpeg","file_size":1048576,"created_at":"2026-11-06T00:00:00Z"})).unwrap()).collect(); cx.notify();  });
        self.library.clone().update(cx, |library, cx| {
            library.playlists = Some(crate::db::PlaylistsBundle {
                liked: crate::db::LikedPlaylist {
                    id: 1,
                    track_ids: vec![1, 3, 5],
                    updated_at: "2026-11-05".into(),
                },
                custom: vec![crate::db::CustomPlaylistSummary {
                    id: 2,
                    name: "Presentation".into(),
                    track_ids: vec![1, 2, 1, 4],
                    updated_at: "2026-11-04".into(),
                }],
            });
            cx.notify();
        });
        for track in &self.library.read(cx).tracks {
            self.thumbs.insert(track.id, None);
            self.hq_thumbs.insert(track.id);
        }
        self.library.clone().update(cx, |library, cx| {
            library.status = Some(crate::db::LibraryStatus {
                tg_user_id: 1,
                revision: 1,
                indexed: 7,
                total: 7,
                complete: true,
                blocking: false,
                processed: 7,
                last_sync_at: Some("2026-11-06T00:00:00Z".into()),
            });
            cx.notify();
        });
        self.library.clone().update(cx, |library, cx| {
            library.cached = [1, 2, 3, 4, 5].into_iter().collect();
            cx.notify();
        });
        self.preferences = Preferences::default();
        let args = std::env::args().collect::<Vec<_>>();
        let mut fixture = None;
        if let Some(path) = args
            .windows(2)
            .find(|a| a[0] == "--preview-fixture")
            .map(|a| &a[1])
        {
            match self.load_visual_fixture(path, cx) {
                Ok(value) => fixture = Some(value),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        let mut session = audio::session::Session::default();
        session.queue = Queue {
            tracks: self.library.read(cx).tracks.clone(),
            cursor: fixture
                .as_ref()
                .and_then(|f| f["player"]["cursor"].as_i64())
                .unwrap_or(if fixture.is_some() { 0 } else { 4 }),
            source: args
                .iter()
                .any(|a| a == "--preview-fixture")
                .then(|| Source {
                    r#type: SourceType::Playlist,
                    playlist_id: serde_json::json!("all"),
                    name: "All tracks".into(),
                    track_ids: self.library.read(cx).tracks.iter().map(|t| t.id).collect(),
                }),
            ..Default::default()
        };
        if fixture
            .as_ref()
            .is_some_and(|f| f["player"]["repeat"] == "all")
        {
            session.preferences.repeat = audio::session::Repeat::All;
        }
        self.playback.update(cx, |state, cx| {
            *state = audio::Snapshot::default();
            cx.notify();
        });
        self.playback.update(cx, |state, cx| {
            state.player = Some(Arc::new(session));
            cx.notify();
        });
        self.playback.update(cx, |state, cx| {
            state.track_id = state
                .player
                .as_ref()
                .and_then(|p| p.queue.current())
                .map(|t| t.id);
            cx.notify();
        });
        self.playback.update(cx, |state, cx| {
            state.duration_seconds = state
                .player
                .as_ref()
                .and_then(|p| p.queue.current())
                .and_then(|t| t.duration)
                .unwrap_or(228) as f64;
            cx.notify();
        });
        if self
            .playback
            .read(cx)
            .track_id
            .is_some_and(|id| self.library.read(cx).cached.contains(&id))
        {
            self.playback.update(cx, |state, cx| {
                state.buffered_ranges = vec![audio::Range {
                    start: 0.,
                    end: state.duration_seconds,
                }];
                cx.notify();
            });
        }
        self.playback.update(cx, |state, cx| {
            state.current_time_seconds = fixture
                .as_ref()
                .and_then(|f| f["player"]["seconds"].as_f64())
                .unwrap_or(83.);
            cx.notify();
        });
    }
}

impl PlayerApp {
    fn load_visual_fixture(
        &mut self,
        path: &str,
        cx: &mut Context<Self>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        use crate::db::{CustomPlaylistSummary, LikedPlaylist, TrackListenStats};
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let tracks = serde_json::from_value(value["tracks"].clone())?;
        self.library.clone().update(cx, |library, cx| {
            library.tracks = tracks;
            cx.notify();
        });
        // A fixture is presentation input: never retain document descriptors.
        self.library.clone().update(cx, |library, cx| {
            for track in &mut library.tracks {
                track.mtproto_document = None;
            }
            cx.notify();
        });
        let text =
            |v: &serde_json::Value, key: &str| v[key].as_str().unwrap_or_default().to_owned();
        let ids = |v: &serde_json::Value| {
            v.as_array()
                .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let user = &value["user"];
        self.user = Some(AuthUser {
            id: user["tg_user_id"].as_i64().unwrap_or(1),
            first_name: text(user, "first_name"),
            last_name: user["last_name"].as_str().map(str::to_owned),
            username: user["username"].as_str().map(str::to_owned),
            phone: None,
        });
        let playlists = value["playlists"].as_array().cloned().unwrap_or_default();
        let liked = playlists.iter().find(|v| v["kind"] == "liked");
        self.library.clone().update(cx, |library, cx| {
            library.playlists = Some(crate::db::PlaylistsBundle {
                liked: liked
                    .map(|p| LikedPlaylist {
                        id: p["id"].as_i64().unwrap_or(1),
                        track_ids: ids(&p["track_ids"]),
                        updated_at: text(p, "updated_at"),
                    })
                    .unwrap_or(LikedPlaylist {
                        id: 1,
                        track_ids: vec![],
                        updated_at: String::new(),
                    }),
                custom: playlists
                    .iter()
                    .filter(|v| v["kind"] == "custom")
                    .map(|p| CustomPlaylistSummary {
                        id: p["id"].as_i64().unwrap_or(2),
                        name: text(p, "name"),
                        track_ids: ids(&p["track_ids"]),
                        updated_at: text(p, "updated_at"),
                    })
                    .collect(),
            });
            cx.notify();
        });
        self.thumbs.clear();
        if let Some(thumbs) = value["thumbnails"].as_object() {
            for (id, path) in thumbs {
                if let (Ok(id), Some(path)) = (id.parse(), path.as_str()) {
                    self.thumbs.insert(id, Some(path.into()));
                }
            }
        }
        // Missing fixture images stay local instead of triggering remote work.
        for t in &self.library.read(cx).tracks {
            self.thumbs.entry(t.id).or_insert(None);
            self.hq_thumbs.insert(t.id);
        }
        self.library.clone().update(cx, |library, cx| {
            library.cached = ids(&value["cached"]).into_iter().collect();
            cx.notify();
        });
        self.avatar = value["avatar"].as_str().map(Into::into);
        self.preferences = serde_json::from_value(value["preferences"].clone()).unwrap_or_default();
        if let Some(stats) = value["stats"].as_array() {
            for v in stats {
                let n = |key: &str| v[key].as_i64().unwrap_or(0);
                self.library.clone().update(cx, |library, cx| {
                    let result = {
                        library.stats.insert(
                            n("track_id"),
                            TrackListenStats {
                                track_id: n("track_id"),
                                starts: n("starts"),
                                qualified_plays: n("qualified_plays"),
                                completes: n("completes"),
                                early_skips: n("early_skips"),
                                total_listened_ms: n("total_listened_ms"),
                                first_played_at_ms: v["first_played_at_ms"].as_i64(),
                                last_played_at_ms: v["last_played_at_ms"].as_i64(),
                                likeness: v["likeness"].as_f64().unwrap_or(0.),
                            },
                        )
                    };
                    cx.notify();
                    result
                });
            }
        }
        self.online = true;
        self.library.update(cx, |library, cx| {
            if let Some(status) = &mut library.status {
                status.tg_user_id = self.user.as_ref().map(|u| u.id).unwrap_or(1);
                status.indexed = library.tracks.len() as i64;
                status.total = status.indexed;
                status.processed = status.indexed;
            }
            cx.notify();
        });
        Ok(value)
    }
}

// Opt-in, account-free frame probe. Never active for normal app launches.
impl PlayerApp {
    pub(super) fn start_preview_performance(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let (Ok(scenario), Ok(output)) = (
            std::env::var("SOUNDGRAMMY_BENCH_SCENARIO"),
            std::env::var("SOUNDGRAMMY_BENCH_READY_FILE"),
        ) {
            let output = std::path::PathBuf::from(output);
            if std::env::args().any(|arg| arg == "--preview-fixture")
                && output.starts_with("/private/tmp")
            {
                self.start_resource_benchmark(scenario, output, window, cx);
            }
            return;
        }
        let Ok(output) = std::env::var("SOUNDGRAMMY_PREVIEW_PERF") else {
            return;
        };
        let path = std::path::PathBuf::from(&output);
        if !std::env::args().any(|arg| arg == "--preview-fixture")
            || !path.starts_with("/private/tmp")
        {
            return;
        }
        let mode =
            std::env::var("SOUNDGRAMMY_PREVIEW_PERF_MODE").unwrap_or_else(|_| "scroll".into());
        if mode == "search" || mode == "drawer" {
            window.resize(size(px(390.), px(844.)));
        }
        let handle = window.window_handle();
        let view = cx.entity();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = handle.update(cx, |_, window, cx| {
                preview_perf_frame(
                    window,
                    cx,
                    view,
                    PreviewPerf {
                        output: path,
                        mode,
                        intervals: vec![],
                        last: None,
                        open_started: None,
                        open_ms: None,
                        frame: 0,
                    },
                );
            });
        })
        .detach();
    }
}

struct PreviewPerf {
    output: std::path::PathBuf,
    mode: String,
    intervals: Vec<f64>,
    last: Option<std::time::Instant>,
    open_started: Option<std::time::Instant>,
    open_ms: Option<f64>,
    frame: usize,
}
fn preview_perf_frame(
    window: &mut Window,
    _: &mut App,
    view: Entity<PlayerApp>,
    mut probe: PreviewPerf,
) {
    window.on_next_frame(move |window, cx| {
        let now = std::time::Instant::now();
        if probe.frame == 0 {
            view.update(cx, |app, cx| {
                app.compact_playlist = true;
                match probe.mode.as_str() {
                    "search" => app.open_panel(Panel::Search, window, cx),
                    "drawer" => app.open_panel(Panel::Navigation, window, cx),
                    "fullscreen" => app.toggle_fullscreen(window,cx),
                    _ => cx.notify(),
                }
            });
            probe.open_started = Some(std::time::Instant::now());
        }
        if probe.frame == 1 {
            probe.open_ms = probe.open_started.map(|start| start.elapsed().as_secs_f64() * 1000.);
        }
        if probe.frame > 30 {
            if let Some(last) = probe.last { probe.intervals.push(now.duration_since(last).as_secs_f64() * 1000.); }
        }
        probe.last = Some(now);
        if probe.frame >= 390 {
            let mut sorted = probe.intervals.clone();
            sorted.sort_by(f64::total_cmp);
            let percentile = |fraction: f64| sorted[((sorted.len()-1) as f64 * fraction).round() as usize];
            let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
            let result = serde_json::json!({
                "accessibility_active":window.is_a11y_active(),
                "window_active":window.is_window_active(),
                "mode": probe.mode, "tracks": view.read(cx).library.read(cx).tracks.len(),
                "window": [f32::from(window.viewport_size().width), f32::from(window.viewport_size().height)],
                "opening_next_frame_ms": probe.open_ms, "mean_frame_ms": mean,
                "fps": 1000. / mean, "p50_frame_ms": percentile(0.5),
                "p95_frame_ms": percentile(0.95), "p99_frame_ms": percentile(0.99),
                "intervals_ms": probe.intervals,
                "method": "display-paced callbacks with one synthetic scroll input per callback; excludes first 30 frames; not input-to-photon latency"
            });
            let _ = std::fs::write(&probe.output, serde_json::to_vec_pretty(&result).unwrap());
            cx.quit();
            return;
        }
        if probe.mode != "drawer" && probe.mode != "fullscreen" && probe.frame > 1 && (!probe.mode.starts_with("scroll-60-input") || probe.frame.is_multiple_of(2)) {
            let viewport = window.viewport_size();
            let position = point(viewport.width * 0.75, viewport.height * 0.5);
            window.dispatch_event(MouseMoveEvent {
                position, pressed_button: None, modifiers: Modifiers::default(),
            }.to_platform_input(), cx);
            window.dispatch_event(ScrollEvent {
                position,
                delta: ScrollDelta::Pixels(point(px(0.), px(-18.))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            }.to_platform_input(), cx);
        }
        probe.frame += 1;
        preview_perf_frame(window, cx, view, probe);
    });
}

/// Benchmark-only control for comparing direct 60 Hz input with resampling.
pub(super) fn direct_pointer_scroll() -> bool {
    static DIRECT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DIRECT.get_or_init(|| {
        std::env::args().any(|arg| arg == "--preview-fixture")
            && std::env::var("SOUNDGRAMMY_PREVIEW_PERF_MODE")
                .ok()
                .as_deref()
                == Some("scroll-60-input-direct")
    })
}

// Resource workloads use the production views, but never a real account or
// playback device. Static workloads schedule no continuous benchmark frames.
impl PlayerApp {
    fn start_resource_benchmark(
        &mut self,
        scenario: String,
        output: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity();
        let start = std::time::Instant::now();
        let handle = window.window_handle();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let startup = serde_json::json!({"library_ready_ms":start.elapsed().as_secs_f64()*1000.,
                    "tracks":view.read(cx).library.read(cx).tracks.len(),"window":[f32::from(window.viewport_size().width),f32::from(window.viewport_size().height)]});
                let _=std::fs::write(output.with_extension("startup.json"),startup.to_string());
                let view = view.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                    let _=handle.update(cx, |_,window,cx| {
                        let opened=std::time::Instant::now();
                        view.update(cx,|app,cx| {
                            app.compact_playlist=true;
                            match scenario.as_str() {
                                "search"|"search-filter" => app.open_panel(Panel::Search,window,cx),
                                "drawer" => app.open_panel(Panel::Navigation,window,cx),
                                "settings" => app.open_panel(Panel::Settings,window,cx),
                                "fullscreen"|"fullscreen-playing" => {
                                    app.toggle_fullscreen(window,cx);
                                    if scenario=="fullscreen-playing" {
                                        app.playback.update(cx, |state, cx| { state.status=audio::Status::Playing; cx.notify(); });
                                        app.bounce_track=app.playback.read(cx).track_id;
                                        let lane=|period:usize| (0..8000).map(|i|if i%period<period/3 {220}else{30}).collect::<Vec<u8>>();
                                        app.bounce_profile=artwork::BounceProfile::new(25,lane(20),lane(20),[lane(20),lane(13),lane(7)]).map(Arc::new);
                                        app.bounce_ready_at=Some(cx.background_executor().now());
                                    }
                                },
                                "minimized" => window.minimize_window(),
                                _ => (),
                            }
                            if scenario=="search-filter" {
                                app.inputs.search.update(cx,|input,cx|input.set_value("005",window,cx));
                                app.library.clone().update(cx, |library, cx| { library.search="005".into(); cx.notify();  });
                                app.library.clone().update(cx, |library, cx| { library.invalidate_track_list(); cx.notify();  });
                            }
                            cx.notify();
                        });
                        let opening=std::rc::Rc::new(std::cell::Cell::new(0.));
                        let opening_write=opening.clone();
                        window.on_next_frame(move |window,_|window.on_next_frame(move |_,_|opening_write.set(opened.elapsed().as_secs_f64()*1000.)));
                        let resource_view=view.clone();
                        let scenario_ready=scenario.clone();
                        let output_ready=output.clone();
                        cx.spawn(async move |cx| {
                            cx.background_executor().timer(Duration::from_secs(2)).await;
                            let _=handle.update(cx, |_,window,cx| {
                                let app=resource_view.read(cx);
                                let ready=serde_json::json!({"scenario":scenario_ready,"tracks":app.library.read(cx).tracks.len(),
                                    "fullscreen":app.fullscreen,"native_fullscreen":window.is_fullscreen(),
                                    "palette_ready":app.fullscreen_cover.is_some(),"playing":app.playback.read(cx).status==audio::Status::Playing,
                                    "window":[f32::from(window.viewport_size().width),f32::from(window.viewport_size().height)],
                                    "opening_callback_ms":opening.get(),"accessibility_active":window.is_a11y_active()});
                                let _=std::fs::write(&output_ready,ready.to_string());
                            });
                        }).detach();
                        if scenario=="scroll"||scenario=="artwork-scroll"||scenario=="search" {
                            resource_scroll(window,std::time::Instant::now(),None);
                        }
                    });
                }).detach();
            });
        });
    }
}
fn resource_scroll(
    window: &mut Window,
    started: std::time::Instant,
    last: Option<std::time::Instant>,
) {
    window.on_next_frame(move |window, cx| {
        let now = std::time::Instant::now();
        let dt = last
            .map_or(1. / 120., |last| now.duration_since(last).as_secs_f32())
            .min(0.1);
        // Same logical scroll speed in both apps, independent of refresh rate.
        let direction = if (now.duration_since(started).as_secs() / 8).is_multiple_of(2) {
            -1.
        } else {
            1.
        };
        let viewport = window.viewport_size();
        let position = point(viewport.width * 0.75, viewport.height * 0.5);
        window.dispatch_event(
            MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            ScrollEvent {
                position,
                delta: ScrollDelta::Pixels(point(px(0.), px(direction * 1080. * dt))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        resource_scroll(window, started, Some(now));
    });
}
