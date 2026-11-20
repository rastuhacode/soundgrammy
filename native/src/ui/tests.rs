//! Exercise production views through GPUI's deterministic native test window.
use super::{model::PlaylistId, Panel, PlayerApp};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    px, size, AppContext, Bounds, Entity, Pixels, Role, Size, TestAppContext, WindowBounds,
    WindowOptions,
};
fn fixture(
    cx: &mut TestAppContext,
    bounds: Size<Pixels>,
) -> (gpui_kit::AnyWindowHandle, Entity<PlayerApp>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        super::bind_shortcuts(cx);
        super::configure_theme(cx);
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: bounds,
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| PlayerApp::new_inner(window, cx, true)),
        )
        .unwrap();
        (window, view)
    })
}
fn track_rows(window: &mut gpui_kit::Window) -> Vec<gpui_kit::test::ElementSnapshot> {
    if window.try_find("track-rows").is_some() {
        window.within("track-rows").find_all_by_role(Role::Row)
    } else {
        vec![]
    }
}
#[gpui_kit::test]
fn desktop_track_rows_fill_the_content_column(cx: &mut TestAppContext) {
    let (window, _) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let rows = track_rows(window);
        assert_eq!(rows.len(), 7);
        for row in rows {
            assert!(
                row.bounds().size.width > px(700.),
                "row width: {:?}",
                row.bounds()
            );
        }
        assert!(window.try_find("track-search").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn large_library_reuses_its_projection_and_mounts_only_viewport_rows(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let initial = view.update(cx, |app, cx| {
        let example = app.library.read(cx).tracks[0].clone();
        for id in 10..5003 {
            let mut track = example.clone();
            track.id = id;
            app.library.clone().update(cx, |library, cx| {
                library.tracks.push(track);
                cx.notify();
            });
            app.thumbs.insert(id, None);
        }
        app.library.clone().update(cx, |library, cx| {
            library.invalidate_track_list();
            cx.notify();
        });
        cx.notify();
        app.library.read(cx).track_list().entries
    });
    assert_eq!(initial.len(), 5000);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("track-row", 0_usize)).is_some());
        assert!(track_rows(window).len() <= 10);
        window.scroll(
            "track-rows",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-4000.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.try_find(("track-row", 0_usize)).is_none());
        assert!((1..=10).contains(&track_rows(window).len()));
        let after = view.read(cx).library.read(cx).track_list().entries;
        assert!(std::sync::Arc::ptr_eq(&initial, &after));
    })
    .unwrap();
}

#[gpui_kit::test]
fn compact_navigation_opens_playlist_and_returns_to_library(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(390.), px(844.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(track_rows(window).is_empty());
        window.click("playlist-all", cx);
        window.render_frame(cx);
        assert!(!track_rows(window).is_empty());
        window.click("back", cx);
        window.render_frame(cx);
        assert!(track_rows(window).is_empty());
    })
    .unwrap();
    assert!(!view.read_with(cx, |v, _| v.compact_playlist));
}
#[gpui_kit::test]
fn compact_playlist_header_preserves_text_and_matches_original_action_breakpoints(
    cx: &mut TestAppContext,
) {
    for width in [320., 359., 360., 390., 479., 480., 639., 640., 767.] {
        let (window, view) = fixture(cx, size(px(width), px(844.)));
        view.update(cx, |app, cx| {
            app.library.update(cx, |library, cx| {
                let playlist = &mut library.playlists.as_mut().unwrap().custom[0];
                playlist.name = "A long playlist name that must truncate horizontally".into();
                playlist.track_ids = vec![1; 478];
                library.select(PlaylistId::Custom(2));
                cx.notify();
            });
            app.compact_playlist = true;
            cx.notify();
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let heading = window.find("playlist-heading").bounds();
            let count = window.find("playlist-track-count").bounds();
            assert!(heading.size.width >= px(60.), "{width}px: {heading:?}");
            assert_eq!(count.size.height, px(16.), "{width}px: {count:?}");
            assert!(count.right() <= window.find("play-playlist").bounds().left());
            assert_eq!(window.try_find("shuffle-playlist").is_some(), width >= 480.);
            assert_eq!(window.try_find("cache-playlist").is_some(), width >= 640.);
            assert_eq!(
                window.try_find("download-playlist").is_some(),
                width >= 640.
            );
            assert_eq!(
                window.try_find("more-playlist-actions").is_some(),
                width < 640.
            );
            assert!(window.find("open-track-search").bounds().right() <= px(width));
            if width < 640. {
                window.click("more-playlist-actions", cx);
                window.render_frame(cx);
                assert!(window.try_find_by_label("Cache playlist").is_some());
                assert!(window.try_find_by_label("Download playlist").is_some());
                assert_eq!(window.try_find_by_label("Shuffle").is_some(), width < 480.);
            }
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn player_artwork_remains_square_when_track_text_competes_with_controls(cx: &mut TestAppContext) {
    for width in [320., 359., 360., 390., 479., 640., 767., 768., 1200.] {
        let (window, view) = fixture(cx, size(px(width), px(844.)));
        view.update(cx, |app, cx| {
            app.playback.update(cx, |snapshot, cx| {
                let track = &mut std::sync::Arc::make_mut(snapshot.player.as_mut().unwrap())
                    .queue
                    .tracks[4];
                track.title =
                    Some("A very long title that has to truncate next to player controls".into());
                track.performer = Some("A very long artist name that also has to truncate".into());
                cx.notify();
            });
            cx.notify();
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let artwork = window.find("player-artwork").bounds();
            let expected = if width < 360. {
                48.
            } else if width < 768. {
                56.
            } else {
                64.
            };
            assert_eq!(
                artwork.size,
                size(px(expected), px(expected)),
                "{width}px: {artwork:?}"
            );
            assert!(artwork.right() <= window.find("previous").bounds().left());
            assert!(window.find("next").bounds().right() <= px(width));
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn player_bar_fills_its_wrapper_to_the_window_bottom(cx: &mut TestAppContext) {
    for width in [320., 390., 767., 768., 1200.] {
        let (window, _) = fixture(cx, size(px(width), px(844.)));
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let bar = window.find("player-bar").bounds();
            assert_eq!(
                bar.bottom(),
                window.viewport_size().height,
                "{width}px: {bar:?}"
            );
            assert_eq!(bar.size.height, px(if width < 768. { 80. } else { 96. }));
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn empty_playlists_hide_track_operations_and_show_contextual_guidance(cx: &mut TestAppContext) {
    for width in [390., 1200.] {
        for (playlist, title) in [
            (PlaylistId::All, "No tracks yet"),
            (PlaylistId::Custom(2), "This playlist is empty"),
            (PlaylistId::Builtin("liked".into()), "No liked tracks yet"),
            (
                PlaylistId::Builtin("popular".into()),
                "No listening history yet",
            ),
            (
                PlaylistId::Builtin("recent".into()),
                "No listening history yet",
            ),
        ] {
            let (window, view) = fixture(cx, size(px(width), px(844.)));
            view.update(cx, |app, cx| {
                match &playlist {
                    PlaylistId::All => app.library.clone().update(cx, |library, cx| {
                        library.tracks.clear();
                        cx.notify();
                    }),
                    PlaylistId::Custom(_) => app.library.clone().update(cx, |library, cx| {
                        library.playlists.as_mut().unwrap().custom[0]
                            .track_ids
                            .clear();
                        cx.notify();
                    }),
                    PlaylistId::Builtin(id) if id == "liked" => {
                        app.library.clone().update(cx, |library, cx| {
                            library.playlists.as_mut().unwrap().liked.track_ids.clear();
                            cx.notify();
                        })
                    }
                    _ => app.library.clone().update(cx, |library, cx| {
                        library.stats.clear();
                        cx.notify();
                    }),
                }
                app.library.clone().update(cx, |library, cx| {
                    library.select(playlist.clone());
                    cx.notify();
                });
                app.library.clone().update(cx, |library, cx| {
                    library.invalidate_track_list();
                    cx.notify();
                });
                app.compact_playlist = true;
                cx.notify();
            });
            cx.update_window(window, |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.find("playlist-empty-title").label(), Some(title));
                assert!(window.try_find("playlist-empty-state").is_some());
                for id in [
                    "play-playlist",
                    "shuffle-playlist",
                    "cache-playlist",
                    "download-playlist",
                    "more-playlist-actions",
                    "track-header",
                    "track-rows",
                    "track-search",
                ] {
                    assert!(
                        window.try_find(id).is_none(),
                        "{width}px {playlist:?}: {id}"
                    );
                }
                assert_eq!(window.try_find("back").is_some(), width < 768.);
                assert_eq!(window.try_find("open-track-search").is_some(), width < 768.);
            })
            .unwrap();
        }
    }
}

#[gpui_kit::test]
fn a_filtered_playlist_keeps_its_toolbar_and_table(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(844.)));
    view.update(cx, |app, cx| {
        app.library.clone().update(cx, |library, cx| {
            library.search = "No matching title or artist exists here".into();
            cx.notify();
        });
        cx.notify();
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("playlist-empty-state").is_none());
        assert!(window.try_find("play-playlist").is_some());
        assert!(window.try_find("track-header").is_some());
        assert!(window.try_find("track-search").is_some());
        assert!(track_rows(window).is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn delayed_volume_title_does_not_close_or_cover_the_range(cx: &mut TestAppContext) {
    use std::time::Duration;
    let (window, _) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.hover("volume", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(1499));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("hover-title").is_none());

        assert!(window.try_find_by_label("Volume").is_some());
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let title = window.find("hover-title");
        assert_eq!(title.label(), Some("Mute"));
        let range = window.find_by_label("Volume").bounds();
        assert!(!title.bounds().intersects(&range));
        assert!(title.bounds().bottom() <= window.viewport_size().height);
        window.hover("exact-volume-chip", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("hover-title").is_none());
        assert!(window.try_find_by_label("Volume").is_some());
        window.click("exact-volume-chip", cx);
        assert!(window
            .try_find_by_label("Exact volume percentage")
            .is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn delayed_titles_reveal_full_playlist_and_track_names(cx: &mut TestAppContext) {
    use std::time::Duration;
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let playlist_name = "A very long playlist name which overflows the sidebar width";
    view.update(cx, |app, cx| {
        app.library.clone().update(cx, |library, cx| {
            library.playlists.as_mut().unwrap().custom[0].name = playlist_name.into();
            cx.notify();
        });
        cx.notify();
    });
    for (target, expected) in [
        ("playlist-2", playlist_name),
        ("player-track-title", "Godhead Interface"),
    ] {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.hover(target, cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(1500));
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("hover-title").label(), Some(expected));
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn modal_opening_clears_visible_and_pending_background_titles(cx: &mut TestAppContext) {
    use std::time::Duration;
    for wait in [500, 1500] {
        let (window, view) = fixture(cx, size(px(1200.), px(800.)));
        cx.update_window(window, |_, window, cx| {
            window.hover("volume", cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(wait));
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.try_find("hover-title").is_some(), wait == 1500);
            view.update(cx, |app, cx| app.open_panel(Panel::Navigation, window, cx));
            window.render_frame(cx);
            assert!(window.try_find("hover-title").is_none());
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(1500));
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("hover-title").is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn choosing_a_playlist_preserves_duplicate_rows_and_clears_search(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.click("playlist-2", cx);
        window.render_frame(cx);
        assert_eq!(track_rows(window).len(), 4);
    })
    .unwrap();
    assert_eq!(
        view.read_with(cx, |v, cx| v.library.read(cx).membership()),
        vec![1, 2, 1, 4]
    );
    assert_eq!(
        view.read_with(cx, |v, cx| v.library.read(cx).selected.clone()),
        PlaylistId::Custom(2)
    );
}
#[gpui_kit::test]
fn settings_dialog_has_a_close_control_and_real_bounce_controls(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.open_panel(Panel::Settings, window, cx));
        window.render_frame(cx);
        assert_eq!(window.find_all_by_role(Role::Dialog).len(), 1);
        window.click("Fullscreen player", cx);
        window.render_frame(cx);
        assert!(window.try_find("bounce-enabled").is_some());
        assert!(window.try_find("keep-awake").is_some());
        window.click("close-panel", cx);
        window.render_frame(cx);
        assert!(window.find_all_by_role(Role::Dialog).is_empty());
    })
    .unwrap();
}
#[gpui_kit::test]
fn search_filters_the_rendered_membership_without_reindexing_it(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.library.clone().update(cx, |library, cx| {
                library.search = "space".into();
                cx.notify();
            });
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(track_rows(window).len(), 2);
    })
    .unwrap();
    assert_eq!(
        view.read_with(cx, |v, cx| v
            .library
            .read(cx)
            .entries()
            .iter()
            .map(|e| e.source_index)
            .collect::<Vec<_>>()),
        vec![0, 6]
    );
}

#[gpui_kit::test]
fn logout_clears_all_account_presentation_and_advances_the_callback_epoch(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |app, cx| {
            let epoch = app.epoch;
            app.jobs.insert("job".into(), "In progress".into());
            app.library.clone().update(cx, |library, cx| {
                let result = { library.selection.insert(0) };
                cx.notify();
                result
            });
            app.event(crate::runtime::Event::AuthRevoked, cx);
            assert_eq!(app.epoch, epoch + 1);
            assert!(app.library.read(cx).tracks.is_empty());
            assert!(app.jobs.is_empty());
            assert!(app.thumbs.is_empty());
            assert!(app.user.is_none());
            assert!(app.command_port.is_none());
        });
    })
    .unwrap();
}
#[gpui_kit::test]
fn out_of_order_snapshots_cannot_reverse_current_playback(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |app, cx| {
            let mut newest = app.playback.read(cx).clone();
            newest.revision = 20;
            newest.current_time_seconds = 100.;
            app.event(crate::runtime::Event::AudioState(newest.clone()), cx);
            newest.revision = 19;
            newest.current_time_seconds = 10.;
            app.event(crate::runtime::Event::AudioState(newest), cx);
            assert_eq!(app.playback.read(cx).current_time_seconds, 100.);
        });
    })
    .unwrap();
}
#[gpui_kit::test]
fn queue_sections_collapse_without_losing_the_current_track(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.open_panel(Panel::Queue, window, cx));
        window.render_frame(cx);
        window.click("queue-section-History", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(!view.read_with(cx, |v, _| v.preferences.history_open));
    assert!(view.read_with(cx, |v, _| v
        .queue_items
        .contains(&super::model::QueueItem::Track(4))));
}
#[gpui_kit::test]
fn full_reindex_opens_confirmation_without_starting_a_sync(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.open_panel(Panel::Navigation, window, cx));
        window.right_click("nav-sync", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let item = window.find_by_label("Reindex entire library");
        window.click(item.path().last().unwrap().clone(), cx);
        window.render_frame(cx);
        assert!(window.try_find("confirm").is_some());
    })
    .unwrap();
    assert!(!view.read_with(cx, |v, _| v.syncing));
}
#[gpui_kit::test]
fn full_reindex_is_disabled_while_syncing(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.syncing = true;
            cx.notify();
        });
        window.click("navigation", cx);
        window.right_click("nav-sync", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let item = window.find_by_label("Reindex entire library");
        window.click(item.path().last().unwrap().clone(), cx);
        window.render_frame(cx);
        assert!(window.try_find("confirm").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn nested_confirmation_dismissal_restores_settings(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.open_panel(Panel::Settings, window, cx));
        window.click("Audio cache", cx);
        window.click("clear-cache", cx);
        window.click("cancel-confirm", cx);
        window.render_frame(cx);
        assert!(window.try_find("clear-cache").is_some());
    })
    .unwrap();
    assert!(view.read_with(cx, |v, _| matches!(v.panel, Some(Panel::Settings))));
}
#[gpui_kit::test]
fn compact_search_has_filtered_rows_and_closing_clears_its_query(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(390.), px(844.)));
    cx.update_window(window, |_, window, cx| {
        window.click("playlist-all", cx);
        window.click("open-track-search", cx);
        view.update(cx, |app, cx| {
            app.inputs
                .search
                .update(cx, |input, cx| input.set_value("rabbit", window, cx));
            app.library.clone().update(cx, |library, cx| {
                library.search = "rabbit".into();
                cx.notify();
            });
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window
                .within("search-results")
                .find_all_by_role(Role::Row)
                .len(),
            1
        );
        window.click("close-track-search", cx);
        window.render_frame(cx);
        assert_eq!(track_rows(window).len(), 7);
    })
    .unwrap();
    assert!(view.read_with(cx, |v, cx| v.library.read(cx).search.is_empty()));
}
fn record_commands(
    view: &Entity<PlayerApp>,
    cx: &mut gpui_kit::App,
) -> std::sync::mpsc::Receiver<crate::audio::session::Command> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let port = super::transport::CommandPort::with_test_handler(cx, move |control| {
        if let crate::audio::Control::Player(command) = control {
            sender.send(*command).unwrap();
        }
        async { Ok(crate::audio::Snapshot::default()) }
    });
    view.update(cx, |app, _| app.command_port = Some(port));
    receiver
}
#[gpui_kit::test]
fn filtered_duplicate_playback_sends_the_clicked_membership_position(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let receiver = cx.update(|cx| record_commands(&view, cx));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.library.clone().update(cx, |library, cx| {
                library.select(PlaylistId::Custom(2));
                cx.notify();
            });
            app.library.clone().update(cx, |library, cx| {
                library.search = "synthetic".into();
                cx.notify();
            });
            cx.notify();
        });
        window.click(("track-row", 2usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let command = receiver.try_recv().unwrap();
    let crate::audio::session::Command::PlayPlaylist { queue, .. } = command else {
        panic!("expected playlist playback")
    };
    assert_eq!(queue.cursor, 1);
    assert_eq!(queue.source_indices, Some(vec![0, 2]));
    assert_eq!(
        queue.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![1, 1]
    );
}
#[gpui_kit::test]
fn keyboard_shortcuts_yield_to_search_inputs(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let receiver = cx.update(|cx| record_commands(&view, cx));
    cx.update_window(window, |_, window, cx| {
        window.click("track-search", cx);
        window.press("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(receiver.try_recv().is_err());
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.focus.focus(window, cx));
        window.press("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(matches!(
        receiver.try_recv().unwrap(),
        crate::audio::session::Command::Toggle
    ));
}
#[gpui_kit::test]
fn notifications_from_previous_accounts_cannot_restore_a_logged_out_player(
    cx: &mut TestAppContext,
) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |app, cx| {
            let snapshot = app.playback.read(cx).clone();
            app.reset(cx);
            app.event(crate::runtime::Event::AudioState(snapshot), cx);
            assert!(app.playback.read(cx).track_id.is_none());
            assert!(app.library.read(cx).tracks.is_empty());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn primary_controls_use_the_existing_soundgrammy_blue(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::configure_theme(cx);
        let theme = gpui_kit::component::Theme::global(cx);
        let background: gpui_kit::Background = theme.tokens.button_primary.into();
        assert_eq!(
            background.as_solid().unwrap(),
            gpui_kit::rgb(0x2aabee).into()
        );
        assert_eq!(theme.slider_bar, gpui_kit::rgb(0x2aabee).into());
    });
}

#[gpui_kit::test]
fn a_full_sync_request_is_not_lost_behind_an_existing_soft_sync(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |app, cx| {
            app.syncing = true;
            app.sync(true, cx);
            assert!(app.pending_full_sync);
            app.reset(cx);
            assert!(!app.pending_full_sync);
        });
    })
    .unwrap();
}
#[gpui_kit::test]
fn sync_events_do_not_duplicate_the_command_completion_hydration(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |app, cx| {
            let before = app.busy;
            app.event(crate::runtime::Event::SyncFinished, cx);
            assert_eq!(app.busy, before);
            assert!(!app.syncing);
        });
    })
    .unwrap();
}
#[gpui_kit::test]
fn lastfm_disconnect_offers_both_pending_scrobble_policies(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.open_panel(
                Panel::Confirm(super::Confirmation::DisconnectLastFm),
                window,
                cx,
            )
        });
        window.render_frame(cx);
        assert!(window.try_find("disconnect-delete").is_some());
        assert!(window.find_by_label("Retain and disconnect").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn renaming_another_playlist_preserves_the_current_view(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.open_panel(Panel::RenamePlaylist(2), window, cx)
        });
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        view.read_with(cx, |v, cx| v.library.read(cx).selected.clone()),
        PlaylistId::All
    );
    assert_eq!(
        view.read_with(cx, |v, cx| v.inputs.name.read(cx).value().to_string()),
        "Presentation"
    );
}
#[gpui_kit::test]
fn switching_playlists_clears_the_visible_search_field(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.inputs
                .search
                .update(cx, |s, cx| s.set_value("rabbit", window, cx));
            app.library.clone().update(cx, |library, cx| {
                library.search = "rabbit".into();
                cx.notify();
            });
            app.select_playlist(PlaylistId::Custom(2), cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    assert!(view.read_with(cx, |v, cx| v.inputs.search.read(cx).value().is_empty()));
}

#[gpui_kit::test]
fn fullscreen_uses_one_control_strip_and_restores_it_after_keyboard_activity(
    cx: &mut TestAppContext,
) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.fullscreen = true;
            app.fullscreen_activity = cx.background_executor().now();
            cx.notify();
        });
        window.render_frame(cx);
        window.find("fullscreen-title");
        assert!(window.try_find("current-track").is_none());
        assert!(window.try_find("play-pause").is_some());
        assert!(window.try_find("fullscreen-queue").is_some());
        window.hover("fullscreen-artwork", cx);
        view.update(cx, |app, cx| {
            app.fullscreen_activity =
                cx.background_executor().now() - std::time::Duration::from_secs(4);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("play-pause").is_none());
        let artwork = window.find("fullscreen-artwork");
        assert!(artwork.visible());
        assert!(artwork.bounds().size.height >= px(100.));
        assert!(artwork.bounds().origin.y >= px(0.));
        assert!(artwork.bounds().bottom() <= px(800.));
        window.hover("fullscreen-artwork", cx);
        window.render_frame(cx);
        assert!(window.try_find("play-pause").is_none());
        window.press("a", cx);
        window.render_frame(cx);
        assert!(window.try_find("play-pause").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn diagnostic_logs_offer_individual_copy_and_clear_history(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.logs.clear();
            app.logs.set_enabled(true);
            app.logs.push("Playback failed".into());
            app.open_panel(Panel::Logs, window, cx);
        });
        window.render_frame(cx);
        assert!(window.try_find(("copy-log", 0_usize)).is_some());
        window.click("clear-logs", cx);
        window.render_frame(cx);
        assert!(window.try_find(("copy-log", 0_usize)).is_none());
    })
    .unwrap();
    assert!(view.read_with(cx, |app, _| app.logs.entries().is_empty()));
}

#[gpui_kit::test]
fn progress_notifications_keep_the_player_description_queue_and_preferences(
    cx: &mut TestAppContext,
) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let before = view.read_with(cx, |app, cx| app.playback.read(cx).player.clone().unwrap());
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            let mut progress = app.playback.read(cx).clone();
            progress.revision += 1;
            progress.player = None;
            progress.current_time_seconds = 84.;
            app.event(crate::runtime::Event::AudioState(progress), cx);
        });
        window.render_frame(cx);
        assert!(window.try_find("current-track").is_some());
        assert_ne!(window.find("play-pause").disabled(), Some(true));
        view.update(cx, |app, cx| app.open_panel(Panel::Queue, window, cx));
        window.render_frame(cx);
        assert!(window.try_find(("queue-track", 4_usize)).is_some());
    })
    .unwrap();
    let after = view.read_with(cx, |app, cx| app.playback.read(cx).player.clone().unwrap());
    assert_eq!(after.queue.tracks.len(), before.queue.tracks.len());
    assert_eq!(after.queue.cursor, before.queue.cursor);
    assert_eq!(
        (
            after.preferences.repeat,
            after.preferences.shuffle,
            after.preferences.mode
        ),
        (
            before.preferences.repeat,
            before.preferences.shuffle,
            before.preferences.mode
        )
    );
}

#[gpui_kit::test]
fn desktop_queue_uses_the_original_popover_width_and_bottom_anchor(cx: &mut TestAppContext) {
    let (window, _) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.click("queue", cx);
        window.render_frame(cx);
        let bounds = window.find("modal").bounds();
        let trigger = window.find("queue").bounds();
        assert_eq!(bounds.size.width, px(320.));
        assert!((f32::from(trigger.top() - bounds.bottom()) - 41.).abs() < 1.);
        assert!(bounds.right() <= px(1192.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigation_is_a_full_height_left_drawer(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.open_panel(Panel::Navigation, window, cx));
        window.render_frame(cx);
        let bounds = window.find("modal").bounds();
        assert_eq!(bounds.origin.x, px(0.));
        assert_eq!(bounds.size.width, px(320.));
        assert_eq!(bounds.bottom(), px(800.));
        assert!(bounds.size.height >= px(760.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn playlist_creation_preserves_new_and_import_tabs(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.click("new-playlist", cx);
        assert!(window.try_find_by_label("Name").is_some());
        assert_eq!(window.find("save-playlist").disabled(), Some(true));
        view.update(cx, |app, cx| {
            app.inputs
                .name
                .update(cx, |input, cx| input.set_value("Example", window, cx))
        });
        window.render_frame(cx);
        assert_ne!(window.find("save-playlist").disabled(), Some(true));
        window.click("Import", cx);
        assert!(window.try_find("import-playlist").is_some());
        assert!(window.try_find_by_label("Name").is_none());
        assert_eq!(window.find("save-playlist").disabled(), Some(true));
        window.click("New", cx);
        assert!(window.try_find_by_label("Name").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn integration_settings_include_the_proxy_form(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.open_panel(Panel::Settings, window, cx));
        window.click("Integrations", cx);
        assert!(window.try_find_by_label("Use MTProto proxy").is_some());
        assert!(window.try_find_by_label("Paste link").is_some());
        assert!(window.try_find_by_label("Secret").is_some());
        assert!(window.try_find("apply-proxy").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn volume_popover_allows_pointer_travel_and_exact_input(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| window.hover("volume", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.hover("exact-volume-chip", cx);
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("exact-volume-chip").is_some());
        window.click("exact-volume-chip", cx);
        assert!(window
            .try_find_by_label("Exact volume percentage")
            .is_some());
    })
    .unwrap();
    assert!(view.read_with(cx, |app, _| app.volume_exact_open));
}

#[gpui_kit::test]
fn sign_in_steps_keep_native_text_input_and_five_code_cells(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.phase = super::Phase::Login;
            app.user = None;
            app.sign_in
                .update(cx, |login, cx| login.set_step(super::LoginStep::Qr, cx));
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("use-phone").is_some());
        window.click("use-phone", cx);
        assert!(window.try_find_by_label("Phone number").is_some());
        view.update(cx, |app, cx| {
            app.sign_in
                .update(cx, |login, cx| login.set_step(super::LoginStep::Code, cx));
            cx.notify();
        });
        window.render_frame(cx);
        for index in 0_usize..5 {
            assert!(window.try_find(("login-code-cell", index)).is_some());
        }
        assert!(window.try_find_by_label("Login code").is_some());
        window.input("12", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |app, cx| app
            .login_inputs(cx)
            .code
            .read(cx)
            .value()
            .to_string()),
        "12"
    );
}
#[gpui_kit::test]
fn download_results_keep_each_result_and_restore_the_previous_panel(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let result = crate::export::PlaylistDownloadResult {
        folder_path: None,
        succeeded: vec![],
        failed: vec![crate::export::PlaylistDownloadFailed {
            track_id: 1,
            title: Some("Track".into()),
            performer: None,
            error: "Offline".into(),
        }],
        warnings: vec![],
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.open_panel(Panel::Settings, window, cx);
            app.download_result(result.clone(), cx);
            app.download_result(result, cx);
        });
        window.render_frame(cx);
        window.click("Not downloaded", cx);
        assert!(window.try_find_by_label("Offline").is_some());
        window.click("dismiss-download-result", cx);
        assert!(window.try_find("dismiss-download-result").is_some());
        window.click("dismiss-download-result", cx);
        assert!(window.try_find("Audio cache").is_some());
    })
    .unwrap();
    assert!(view.read_with(cx, |app, _| app.download_results.is_empty()));
}

#[gpui_kit::test]
fn library_artist_header_aligns_with_the_track_column(cx: &mut TestAppContext) {
    let (window, _) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let header = window.find("sort-artist").bounds().left();
        let row = window.find(("track-artist", 0_usize)).bounds().left();
        assert!(
            (f32::from(header - row)).abs() < 4.,
            "header {header:?}, row {row:?}"
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn compact_player_uses_a_drawer_and_keeps_it_behind_the_queue(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(390.), px(844.)));
    cx.update_window(window, |_, window, cx| {
        window.click("current-track", cx);
        assert!(window.try_find("close-compact-player").is_some());
        window.click("compact-player-queue", cx);
        assert!(window.try_find("close-compact-player").is_some());
        assert!(window.try_find("close-panel").is_some());
        window.click("close-panel", cx);
        window.click("close-compact-player", cx);
        assert!(window.try_find("playlist-all").is_some());
    })
    .unwrap();
    assert!(view.read_with(cx, |app, _| !app.fullscreen && app.panel.is_none()));
}

#[gpui_kit::test]
fn minimum_sidebar_width_keeps_its_header_on_one_line(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.preferences.sidebar_width = 220.;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("Library").bounds().size.height <= px(32.));
        assert!(window.find("playlist-all").bounds().top() <= px(100.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn backdrop_press_dismisses_without_playing_or_activating_the_library(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;
    use gpui_kit::{point, MouseButton};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    for panel in [Panel::Settings, Panel::Navigation, Panel::NewPlaylist] {
        cx.update_window(window, |_, window, cx| {
            let row = window.find(("track-row", 0_usize)).bounds();
            let target = point(row.right() - px(80.), row.top() + px(20.));
            view.update(cx, |app, cx| app.open_panel(panel.clone(), window, cx));
            window.render_frame(cx);
            let before = view.read(cx).busy;
            window.pointer_down(target, MouseButton::Right, cx);
            window.pointer_up(target, MouseButton::Right, cx);
            assert!(
                view.read(cx).panel.is_some(),
                "right press must not dismiss"
            );
            window.render_frame(cx);
            assert!(window.find_all_by_role(Role::Menu).is_empty());
            window.pointer_down(target, MouseButton::Left, cx);
            window.pointer_up(target, MouseButton::Left, cx);
            assert!(view.read(cx).panel.is_none(), "outside press must dismiss");
            assert_eq!(
                view.read(cx).busy,
                before,
                "outside press must not dispatch playback"
            );
            assert!(view.read(cx).command_port.is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn filter_trigger_toggles_and_escape_restores_trigger_focus(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.click("playlist-filters", cx);
        assert!(window.try_find("playlist-sort-recency").is_some());
        window.click("playlist-filters", cx);
        assert!(window.try_find("playlist-sort-recency").is_none());
        window.click("playlist-filters", cx);
        window.press("escape", cx);
        assert!(window.try_find("playlist-sort-recency").is_none());
        assert!(view.read(cx).panel.is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn sort_arrows_follow_primary_and_secondary_sort_cycles(cx: &mut TestAppContext) {
    let (window, _) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.find("sort-icon-title").label(), Some("Unsorted"));
        window.click("sort-title", cx);
        assert_eq!(window.find("sort-icon-title").label(), Some("Ascending"));
        window.click("sort-title", cx);
        assert_eq!(window.find("sort-icon-title").label(), Some("Descending"));
        window.click("sort-title", cx);
        assert_eq!(window.find("sort-icon-title").label(), Some("Unsorted"));
        window.click("sort-title", cx);
        window.click_with_modifiers(
            "sort-artist",
            gpui_kit::Modifiers {
                shift: true,
                ..Default::default()
            },
            cx,
        );
        assert_eq!(window.find("sort-icon-title").label(), Some("Ascending"));
        assert_eq!(window.find("sort-icon-artist").label(), Some("Ascending"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn selection_replaces_playback_actions_and_uses_a_mixed_select_all(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.selection_mode = true;
            app.library.clone().update(cx, |library, cx| {
                library.select_entry(0, false);
                cx.notify();
            });
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("play-playlist").is_none());
        assert!(window.try_find("select-all").is_none());
        assert!(window.try_find("bulk-actions").is_some());
        assert_eq!(window.find("select-all-tracks").indeterminate(), Some(true));
        window.click("select-all-tracks", cx);
        assert_eq!(view.read(cx).library.read(cx).selection.len(), 7);
        window.click("select-all-tracks", cx);
        assert!(view.read(cx).library.read(cx).selection.is_empty());
        window.click("exit-selection", cx);
        assert!(window.try_find("play-playlist").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn empty_snapshot_closes_queue_and_the_compact_player(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(390.), px(844.)));
    cx.update_window(window, |_, window, cx| {
        window.click("current-track", cx);
        window.click("compact-player-queue", cx);
        view.update(cx, |app, cx| {
            let mut snapshot = app.playback.read(cx).clone();
            let mut player = (**snapshot.player.as_ref().unwrap()).clone();
            player.queue = Default::default();
            snapshot.player = Some(std::sync::Arc::new(player));
            snapshot.track_id = None;
            app.accept_snapshot(snapshot, cx);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("close-panel").is_none());
        assert!(window.try_find("close-compact-player").is_none());
        assert!(!view
            .read(cx)
            .panel_stack
            .iter()
            .any(|p| matches!(p, Panel::Queue)));
    })
    .unwrap();
}

#[gpui_kit::test]
fn queue_scroll_does_not_scroll_the_library(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        let row = window.find(("track-row", 0_usize)).bounds();
        window.click("queue", cx);
        let before = view.read(cx).queue_list.logical_scroll_top();
        window.scroll(
            "modal",
            gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., -30.)),
            cx,
        );
        window.render_frame(cx);
        assert_ne!(view.read(cx).queue_list.logical_scroll_top(), before);
        assert_eq!(window.find(("track-row", 0_usize)).bounds(), row);
    })
    .unwrap();
}

#[gpui_kit::test]
fn non_reorderable_tracks_do_not_start_a_drag(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;
    let (window, _) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        let start = window.find(("track-row", 0_usize)).bounds().center();
        window.pointer_down(start, gpui_kit::MouseButton::Left, cx);
        window.pointer_move(
            start + gpui_kit::point(px(0.), px(90.)),
            Some(gpui_kit::MouseButton::Left),
            cx,
        );
        assert!(!cx.has_active_drag());
        window.pointer_up(
            start + gpui_kit::point(px(0.), px(90.)),
            gpui_kit::MouseButton::Left,
            cx,
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn all_tracks_has_no_invented_playlist_menu(cx: &mut TestAppContext) {
    let (window, _) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.hover("playlist-all", cx);
        assert!(window.try_find("playlist-options-all").is_none());
        window.right_click("playlist-all", cx);
        assert!(window.find_all_by_role(Role::Menu).is_empty());
        assert!(window.try_find_by_label("Open playlist").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn sortable_tracks_use_a_row_preview_shift_neighbours_and_cancel_with_escape(
    cx: &mut TestAppContext,
) {
    use gpui_kit::test::TestEventExt;
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.click("playlist-2", cx);
        let before = view.read(cx).library.read(cx).membership();
        let start = window.find(("track-row", 0_usize)).bounds().center();
        let target = window.find(("track-row", 1_usize)).bounds().center();
        window.pointer_down(start, gpui_kit::MouseButton::Left, cx);
        window.pointer_move(
            start + gpui_kit::point(px(0.), px(10.)),
            Some(gpui_kit::MouseButton::Left),
            cx,
        );
        window.pointer_move(target, Some(gpui_kit::MouseButton::Left), cx);
        assert!(cx.has_active_drag());
        let preview = window.find("sortable-drag-preview").bounds();
        assert!(preview.size.width > px(700.));
        assert_eq!(preview.size.height, px(70.));
        assert_eq!(view.read(cx).sort_drag.unwrap().over, 1);
        window.press("escape", cx);
        assert!(!cx.has_active_drag());
        assert!(window.try_find("sortable-drag-preview").is_none());
        assert_eq!(view.read(cx).library.read(cx).membership(), before);
    })
    .unwrap();
}

#[gpui_kit::test]
fn playlist_dialog_focuses_name_and_restores_the_previous_focus(cx: &mut TestAppContext) {
    use gpui_kit::Focusable;
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        let previous = window.focused(cx);
        view.update(cx, |app, cx| app.open_panel(Panel::NewPlaylist, window, cx));
        window.render_frame(cx);
        assert!(view
            .read(cx)
            .inputs
            .name
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
        window.click("close-panel", cx);
        assert_eq!(window.focused(cx), previous);
    })
    .unwrap();
}

#[gpui_kit::test]
fn playlist_tabs_support_arrow_key_selection(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.click("new-playlist", cx);
        // The dialog starts at Name. Use the window's tab order to enter the
        // Kit tab list, which supplies its own focus handle.
        for _ in 0..8 {
            if window.find("New").focused() == Some(true) {
                break;
            }
            window.press("shift-tab", cx);
        }
        assert_eq!(window.find("New").focused(), Some(true));
        window.press("right", cx);
        assert!(view.read(cx).new_playlist_import);
        assert_eq!(window.find("Import").focused(), Some(true));
        assert!(window.try_find("import-playlist").is_some());
        window.press("left", cx);
        assert!(!view.read(cx).new_playlist_import);
        assert!(window.try_find_by_label("Name").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn seek_hover_preview_is_centered_on_the_pointer_and_does_not_seek(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        let bar = window.find("playback-progress").bounds();
        let target = gpui_kit::point(bar.left() + bar.size.width * 0.25, bar.center().y);
        let before = view.read(cx).playback.read(cx).current_time_seconds;
        window.pointer_move(target, None, cx);
        window.render_frame(cx);
        let tooltip = window.find("seek-tooltip").bounds();
        assert!((f32::from(tooltip.center().x - target.x)).abs() < 2.);
        assert_eq!(view.read(cx).playback.read(cx).current_time_seconds, before);
        let preview = view.read(cx).seek_preview.unwrap();
        assert!((preview.0 - 0.25).abs() < 0.01);
    })
    .unwrap();
}

#[gpui_kit::test]
fn seek_rail_is_flush_and_pointer_positions_send_finite_seconds(cx: &mut TestAppContext) {
    use gpui_kit::{point, test::TestEventExt, MouseButton};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let (tx, rx) = std::sync::mpsc::channel();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.playback.update(cx, |state, cx| {
                state.duration_seconds = 200.;
                cx.notify();
            });
            app.playback.update(cx, |state, cx| {
                state.current_time_seconds = 50.;
                cx.notify();
            });
            app.playback.update(cx, |state, cx| {
                state.attempt_id = Some("seek-test".into());
                cx.notify();
            });
            app.command_port = Some(super::transport::CommandPort::with_test_handler(
                cx,
                move |control| {
                    if let crate::audio::Control::SeekAttempt(seconds, attempt) = control {
                        tx.send((seconds, attempt)).unwrap();
                    }
                    async { Ok(crate::audio::Snapshot::default()) }
                },
            ));
            cx.notify();
        });
        window.render_frame(cx);
        let bar = window.find("playback-progress").bounds();
        assert_eq!(bar.top(), window.find("player-bar").bounds().top());
        assert_eq!(bar.size.height, px(4.));
        assert!(
            (window.find("seek-fill").bounds().size.width - bar.size.width * 0.25).abs() < px(1.)
        );

        let at = |fraction| point(bar.left() + bar.size.width * fraction, bar.center().y);
        window.pointer_move(at(0.33), None, cx);
        window.pointer_down(at(0.33), MouseButton::Left, cx);
        window.pointer_up(at(0.33), MouseButton::Left, cx);
        window.pointer_move(at(0.25), None, cx);
        window.pointer_down(at(0.25), MouseButton::Left, cx);
        window.pointer_move(at(0.45), Some(MouseButton::Left), cx);
        window.pointer_move(at(0.75), Some(MouseButton::Left), cx);
        window.pointer_up(at(0.75), MouseButton::Left, cx);
        assert!(view.read(cx).scrubbing.is_none());
    })
    .unwrap();
    cx.run_until_parked();
    for expected in [66., 150.] {
        let (seconds, attempt) = rx.try_recv().unwrap();
        assert!((seconds - expected).abs() < 0.1, "seek target {seconds}");
        assert_eq!(attempt.as_deref(), Some("seek-test"));
    }
}

#[gpui_kit::test]
fn clicking_volume_rail_changes_percent_without_dismissing(cx: &mut TestAppContext) {
    use gpui_kit::{point, test::TestEventExt, MouseButton};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            let snapshot = app.playback.read(cx).clone();
            app.command_port = Some(super::transport::CommandPort::with_test_handler(
                cx,
                move |_| {
                    let snapshot = snapshot.clone();
                    async { Ok(snapshot) }
                },
            ));
        });
        window.hover("volume", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let slider = window.find_by_label("Volume").bounds();
        let target = point(slider.center().x, slider.top() + slider.size.height * 0.25);
        window.pointer_move(target, None, cx);
        window.pointer_down(target, MouseButton::Left, cx);
        window.pointer_up(target, MouseButton::Left, cx);
        let state = view.read(cx).volume_slider.read(cx);
        assert_eq!(
            state.value().start(),
            75.,
            "volume bounds {:?}",
            state.bounds()
        );
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("volume-popover").is_some());
        assert_eq!(view.read(cx).preferences.volume, 75.);
    })
    .unwrap();
}

#[gpui_kit::test]
fn track_search_clear_restores_the_unfiltered_list(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let count = cx
        .update_window(window, |_, window, cx| {
            assert!(window.try_find("clean").is_none());
            let count = track_rows(window).len();
            window.click("track-search", cx);
            window.input("space", cx);
            count
        })
        .unwrap();
    cx.update_window(window, |_, window, cx| {
        assert!(track_rows(window).len() < count);
        window.click("clean", cx);
    })
    .unwrap();
    cx.update_window(window, |_, window, _| {
        assert_eq!(track_rows(window).len(), count);
        assert!(window.try_find("clean").is_none());
    })
    .unwrap();
    assert!(view.read_with(cx, |app, cx| app.library.read(cx).search.is_empty()));
}

#[gpui_kit::test]
fn modal_and_popover_retain_visuals_only_during_exit(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update(|cx| cx.set_reduce_motion(false));
    for (trigger, close, visual) in [
        ("new-playlist", "close-panel", "modal"),
        (
            "playlist-filters",
            "playlist-filters",
            "playlist-sort-recency",
        ),
    ] {
        cx.update_window(window, |_, window, cx| window.click(trigger, cx))
            .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(120));
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(visual).is_some());
            if trigger == "new-playlist" {
                window.press("escape", cx);
            } else {
                window.click(close, cx);
            }
            assert!(view.read(cx).panel.is_none());
            assert!(window.try_find(visual).is_some());
        })
        .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(120));
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(visual).is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn now_playing_equalizer_animates_and_respects_reduced_motion(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update(|cx| {
        cx.set_reduce_motion(false);
        view.update(cx, |app, cx| {
            app.playback.update(cx, |state, cx| {
                std::sync::Arc::make_mut(state.player.as_mut().unwrap()).is_playing = true;
                cx.notify();
            });
            cx.notify();
        });
    });
    let heights = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            [0_usize, 1, 2].map(|i| window.find(("equalizer-bar", i)).bounds().size.height)
        })
        .unwrap()
    };
    let before = heights(cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(350));
    assert_ne!(heights(cx), before);
    cx.update(|cx| cx.set_reduce_motion(true));
    let static_bars = heights(cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(350));
    assert_eq!(heights(cx), static_bars);
}

#[gpui_kit::test]
fn equalizer_frames_reuse_inactive_track_rows(cx: &mut TestAppContext) {
    cx.disable_accessibility();
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update(|cx| {
        cx.set_reduce_motion(false);
        view.update(cx, |app, cx| {
            app.playback.update(cx, |state, cx| {
                std::sync::Arc::make_mut(state.player.as_mut().unwrap()).is_playing = true;
                cx.notify();
            });
            cx.notify();
        });
    });
    let before = cx
        .update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.read(cx).inactive_row_renders.set(0);
            window.find(("equalizer-bar", 0_usize)).bounds().size.height
        })
        .unwrap();

    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(350));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        // Render the frame GPUI scheduled without forcing a global refresh.
        window.draw(cx).clear(cx);
        assert_ne!(
            window.find(("equalizer-bar", 0_usize)).bounds().size.height,
            before,
        );
        assert_eq!(view.read(cx).inactive_row_renders.get(), 0);
        assert_eq!(track_rows(window).len(), 7);
    })
    .unwrap();
}

#[gpui_kit::test]
fn overlapping_seek_hit_area_does_not_activate_the_track_row(cx: &mut TestAppContext) {
    use gpui_kit::{point, test::TestEventExt, MouseButton};
    let (window, view) = fixture(cx, size(px(1200.), px(680.)));
    let (tx, rx) = std::sync::mpsc::channel();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            let snapshot = app.playback.read(cx).clone();
            app.command_port = Some(super::transport::CommandPort::with_test_handler(
                cx,
                move |control| {
                    tx.send(control).unwrap();
                    let snapshot = snapshot.clone();
                    async { Ok(snapshot) }
                },
            ));
        });
        window.render_frame(cx);
        let rail = window.find("playback-progress").bounds();
        let target = point(rail.left() + rail.size.width * 0.6, rail.top() - px(6.));
        assert!(track_rows(window)
            .iter()
            .any(|row| row.bounds().contains(&target)));
        window.pointer_move(target, None, cx);
        assert!(window.try_find("seek-tooltip").is_some());
        window.pointer_down(target, MouseButton::Left, cx);
        window.pointer_up(target, MouseButton::Left, cx);
    })
    .unwrap();
    cx.run_until_parked();
    let command = rx.try_recv().unwrap();
    assert!(matches!(command, crate::audio::Control::SeekAttempt(..)));
    assert!(rx.try_recv().is_err(), "row also handled the click");
}

#[gpui_kit::test]
fn volume_drag_outlives_hover_delay_and_closes_only_after_release_and_exit(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{point, test::TestEventExt, MouseButton};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        view.update(cx, |app, cx| {
            let snapshot = app.playback.read(cx).clone();
            app.command_port = Some(super::transport::CommandPort::with_test_handler(
                cx,
                move |_| {
                    let snapshot = snapshot.clone();
                    async { Ok(snapshot) }
                },
            ));
        });
        window.hover("volume", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let (target, outside) = cx
        .update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let slider = window.find_by_label("Volume").bounds();
            let x = slider.center().x;
            let start = point(x, slider.top() + slider.size.height * 0.4);
            let target = point(x, slider.top() + slider.size.height * 0.2);
            let outside = point(x - px(80.), target.y);
            window.pointer_move(start, None, cx);
            window.pointer_down(start, MouseButton::Left, cx);
            window.pointer_move(target, Some(MouseButton::Left), cx);
            window.pointer_move(outside, Some(MouseButton::Left), cx);
            assert!(cx.has_active_drag());
            (target, outside)
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find("volume-popover").is_some(),
            "the popup closed during the drag"
        );
        window.pointer_move(target, Some(MouseButton::Left), cx);
        window.pointer_up(target, MouseButton::Left, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find("volume-popover").is_some(),
            "release inside dismissed the popup"
        );
        assert_eq!(view.read(cx).preferences.volume, 80.);
        window.pointer_move(outside, None, cx);
        // A focused slider now keeps its parent popup open, like the exact
        // input. Move focus away as well before expecting idle dismissal.
        view.update(cx, |app, cx| app.focus.focus(window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("volume-popover").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn fullscreen_controls_slide_and_fade_after_idle_then_reveal_on_pointer_move(
    cx: &mut TestAppContext,
) {
    use gpui_kit::test::TestEventExt;
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        cx.set_reduce_motion(false);
        view.update(cx, |app, cx| {
            app.fullscreen = true;
            app.show_fullscreen_controls(window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(2999));
    let visible_top = cx
        .update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(!view.read(cx).fullscreen_cursor_hidden);
            window.find("fullscreen-controls-panel").bounds().top()
        })
        .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(1));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(view.read(cx).fullscreen_cursor_hidden);
        assert!(window.try_find("fullscreen-controls-panel").is_some());
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(250));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let fading_top = window.find("fullscreen-controls-panel").bounds().top();
        assert!(fading_top > visible_top && fading_top < visible_top + px(16.));
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(250));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("fullscreen-controls-panel").is_none());
        window.pointer_move(gpui_kit::point(px(600.), px(760.)), None, cx);
        assert!(!view.read(cx).fullscreen_cursor_hidden);
        assert!(window.try_find("fullscreen-controls-panel").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn fullscreen_cursor_remains_visible_for_interactions_and_restores_on_exit(
    cx: &mut TestAppContext,
) {
    use std::time::Duration;
    #[cfg(target_os = "macos")]
    let original_cursor_mode = cx.update(|cx| cx.cursor_hide_mode());
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        view.update(cx, |app, cx| {
            app.fullscreen = true;
            app.show_fullscreen_controls(window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    for interaction in 0..3 {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(view.read(cx).fullscreen_cursor_hidden);
            view.update(cx, |app, cx| {
                match interaction {
                    0 => app.panel = Some(Panel::Queue),
                    1 => app.volume_hover = true,
                    _ => app.scrubbing = Some(50.),
                }
                cx.notify();
            });
            window.render_frame(cx);
            assert!(!view.read(cx).fullscreen_cursor_hidden);
            view.update(cx, |app, cx| {
                app.panel = None;
                app.volume_hover = false;
                app.scrubbing = None;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(view.read(cx).fullscreen_cursor_hidden);
        })
        .unwrap();
    }
    cx.update_window(window, |_, window, cx| {
        window.press("a", cx);
        window.render_frame(cx);
        assert!(!view.read(cx).fullscreen_cursor_hidden);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(view.read(cx).fullscreen_cursor_hidden);
        view.update(cx, |app, cx| {
            app.fullscreen = false;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(!view.read(cx).fullscreen_cursor_hidden);
        assert!(window.try_find("player-bar").is_some());
        #[cfg(target_os = "macos")]
        assert_eq!(cx.cursor_hide_mode(), original_cursor_mode);
    })
    .unwrap();
}

#[gpui_kit::test]
fn fullscreen_bounce_tracks_audio_profile_and_releases_when_paused(cx: &mut TestAppContext) {
    use crate::audio::Status;
    use std::{sync::Arc, time::Duration};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        cx.set_reduce_motion(false);
        view.update(cx, |app, cx| {
            app.fullscreen = true;
            app.show_fullscreen_controls(window, cx);
            app.playback.update(cx, |state, cx| {
                state.status = Status::Playing;
                cx.notify();
            });
            app.playback.update(cx, |state, cx| {
                state.current_time_seconds = 0.;
                cx.notify();
            });
            app.preferences.bounce.enabled = true;
            app.bounce_track = app.playback.read(cx).track_id;
            let bass: Vec<u8> = (0..160)
                .map(|i| if i >= 40 && i % 8 < 3 { 255 } else { 0 })
                .collect();
            app.bounce_profile = Some(Arc::new(
                super::artwork::BounceProfile::new(
                    25,
                    [vec![0; 40], vec![255; 120]].concat(),
                    bass.clone(),
                    [bass, vec![0; 160], vec![0; 160]],
                )
                .unwrap(),
            ));
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(900));
    let cover = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            window.find("fullscreen-bounce-cover").bounds()
        })
        .unwrap()
    };
    let quiet = cover(cx);
    cx.update(|cx| {
        view.update(cx, |app, cx| {
            app.playback.update(cx, |state, cx| {
                state.current_time_seconds = 2.4;
                cx.notify();
            });
            cx.notify();
        })
    });
    cover(cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    let beat = cover(cx);
    assert!(beat.size.width > quiet.size.width + px(10.));
    assert!(beat.center().y < quiet.center().y);
    cx.update(|cx| {
        view.update(cx, |app, cx| {
            app.playback.update(cx, |state, cx| {
                state.status = Status::Paused;
                cx.notify();
            });
            cx.notify();
        })
    });
    cover(cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    let releasing = cover(cx);
    assert!(releasing.size.width < beat.size.width);
    assert!(releasing.size.width > quiet.size.width);
    cx.update(|cx| cx.set_reduce_motion(true));
    assert_eq!(cover(cx).size.width, quiet.size.width);
}

#[gpui_kit::test]
fn playback_indicators_follow_intent_through_loading_buffering_and_seeking(
    cx: &mut TestAppContext,
) {
    use crate::audio::Status;
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.playback.update(cx, |state, cx| {
                state.status = Status::Loading;

                cx.notify();
            });
            app.playback.update(cx, |state, cx| {
                state.initial_loading = true;

                cx.notify();
            });
            app.playback.update(cx, |state, cx| {
                std::sync::Arc::make_mut(state.player.as_mut().unwrap()).is_playing = true;

                cx.notify();
            });

            cx.notify();

            app.update_effects(window, cx);
        });
        window.render_frame(cx);
        assert!(window.try_find("seek-initial-loading").is_none());
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("seek-initial-loading").is_some());
        assert_eq!(window.find("play-pause").label(), Some("Pause"));
        assert!(window.try_find(("equalizer-bar", 0_usize)).is_some());

        for (status, initial_loading, seeking, playing) in [
            (Status::Playing, false, false, true),
            (Status::Buffering, false, false, true),
            (Status::Buffering, false, true, true),
            // Seeking to zero can restart the pipeline with its initial-loading flag set.
            (Status::Loading, true, true, true),
            (Status::Paused, false, false, false),
            (Status::Buffering, false, true, false),
        ] {
            view.update(cx, |app, cx| {
                app.playback.update(cx, |state, cx| {
                    state.status = status;
                    cx.notify();
                });
                app.playback.update(cx, |state, cx| {
                    state.initial_loading = initial_loading;
                    cx.notify();
                });
                app.playback.update(cx, |state, cx| {
                    state.seeking = seeking;
                    cx.notify();
                });
                app.playback.update(cx, |state, cx| {
                    std::sync::Arc::make_mut(state.player.as_mut().unwrap()).is_playing = playing;
                    cx.notify();
                });

                cx.notify();

                app.update_effects(window, cx);
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("play-pause").label(),
                Some(if playing { "Pause" } else { "Play" })
            );
            assert_eq!(
                window.try_find(("equalizer-bar", 0_usize)).is_some(),
                playing
            );
            assert!(window.try_find("seek-initial-loading").is_none());
            assert!(view.read(cx).loading_started.is_none());
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn disabling_statistics_hides_smart_playlists_and_returns_to_all_without_erasing_history(
    cx: &mut TestAppContext,
) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.click("playlist-popular", cx);
        view.update(cx, |app, cx| {
            let history = crate::db::TrackListenStats {
                track_id: 1,
                starts: 2,
                qualified_plays: 1,
                completes: 0,
                early_skips: 0,
                total_listened_ms: 90000,
                first_played_at_ms: Some(1),
                last_played_at_ms: Some(2),
                likeness: 1.0,
            };
            app.library.clone().update(cx, |library, cx| {
                let result = { library.stats.insert(1, history) };
                cx.notify();
                result
            });
            app.set_statistics_enabled(false, cx);
        });
        window.render_frame(cx);
        assert!(window.try_find("playlist-popular").is_none());
        assert!(window.try_find("playlist-recent").is_none());
        assert_eq!(view.read(cx).library.read(cx).selected, PlaylistId::All);
        assert_eq!(view.read(cx).library.read(cx).stats[&1].qualified_plays, 1);
        view.update(cx, |app, cx| app.set_statistics_enabled(true, cx));
        window.render_frame(cx);
        assert!(window.try_find("playlist-popular").is_some());
        assert!(window.try_find("playlist-recent").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn thumbnail_download_progress_is_visible_and_cleared_by_cache_completion(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.download_progress.insert(1, 0.42);
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.find(("thumbnail-progress", 1_usize)).label(),
            Some("Downloading 42%")
        );
        view.update(cx, |app, cx| {
            app.event(
                crate::runtime::Event::CacheChanged(crate::cache::CacheChanged {
                    track_ids: vec![1],
                    cached: true,
                    cleared: false,
                }),
                cx,
            )
        });
        window.render_frame(cx);
        assert!(window.try_find(("thumbnail-progress", 1_usize)).is_none());
        assert!(view.read(cx).library.read(cx).cached.contains(&1));
    })
    .unwrap();
}

#[gpui_kit::test]
fn compact_search_preserves_player_virtualizes_and_opens_context_menu(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(390.), px(844.)));
    view.update(cx, |app, cx| {
        let example = app.library.read(cx).tracks[0].clone();
        for id in 10..5003 {
            let mut track = example.clone();
            track.id = id;
            app.library.clone().update(cx, |library, cx| {
                library.tracks.push(track);
                cx.notify();
            });
            app.thumbs.insert(id, None);
        }
        app.library.clone().update(cx, |library, cx| {
            library.invalidate_track_list();
            cx.notify();
        });
        app.panel = Some(Panel::Search);
        cx.notify();
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let overlay = window.find("compact-search-overlay").bounds();
        assert_eq!(overlay.size.width, px(390.));
        assert_eq!(overlay.bottom(), px(748.));
        assert!(window.try_find("tracks-table").is_none());
        let rows = window.within("search-results").find_all_by_role(Role::Row);
        assert!((1..=10).contains(&rows.len()), "{} rows", rows.len());
        window.right_click(("track-row", 0_usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!window.find_all_by_role(Role::Menu).is_empty());
        assert!(window.try_find_by_label("Play next").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn embedded_cover_survives_remote_completion(cx: &mut TestAppContext) {
    let (_, view) = fixture(cx, size(px(1200.), px(800.)));
    view.update(cx, |app, _| {
        app.accept_thumbnail(5, "cover.full.jpg".into());
        app.accept_thumbnail(5, "cover.embed.jpg".into());
        app.accept_thumbnail(5, "cover.full.jpg".into());
        assert_eq!(
            app.thumbs.get(&5).and_then(Option::as_deref),
            Some(std::path::Path::new("cover.embed.jpg"))
        );
    });
}

#[gpui_kit::test]
fn audio_progress_updates_preserve_order_without_rerendering_rows(cx: &mut TestAppContext) {
    cx.disable_accessibility();
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.read(cx).inactive_row_renders.set(0);
        view.update(cx, |app, cx| {
            let mut heartbeat = app.playback.read(cx).clone();
            heartbeat.player = None;
            heartbeat.revision = 100;
            app.event(crate::runtime::Event::AudioState(heartbeat), cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(view.read(cx).playback.read(cx).revision, 100);
        assert_eq!(view.read(cx).inactive_row_renders.get(), 0);
        view.update(cx, |app, cx| {
            let mut buffer = app.playback.read(cx).clone();
            buffer.player = None;
            buffer.revision = 101;
            buffer.current_time_seconds = 85.;
            buffer.buffered_ranges.clear();
            app.event(crate::runtime::Event::AudioState(buffer), cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.draw(cx).clear(cx);
        assert!(view.read(cx).playback.read(cx).buffered_ranges.is_empty());
        assert_eq!(view.read(cx).inactive_row_renders.get(), 0);
        let snapshot = view.read(cx).playback.read(cx);
        let expected = (85. / snapshot.duration_seconds * 100.) as f32;
        assert!((view.read(cx).seek_slider.read(cx).value().start() - expected).abs() < 0.01);
    })
    .unwrap();
}

#[gpui_kit::test]
fn fullscreen_waits_for_prepared_fallback_then_opens_with_palette_and_cover(
    cx: &mut TestAppContext,
) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fallback.png");
    image::RgbaImage::from_pixel(64, 64, image::Rgba([180, 25, 40, 255]))
        .save(&path)
        .unwrap();
    let prepared = super::artwork::prepare(&path).unwrap();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.preferences.bounce.enabled = false;
            app.playing_artwork_path = Some(path.clone());
            app.thumbs.insert(5, Some(path.clone()));
            app.prepared_artwork_track = Some(5);
            app.preparing_artwork_path = Some(path.clone());
            app.toggle_fullscreen(window, cx);
            assert!(app.pending_fullscreen);
            assert!(!app.fullscreen);
            app.dismiss_layer(window, cx);
            assert!(!app.pending_fullscreen);
            app.toggle_fullscreen(window, cx);
            assert!(app.pending_fullscreen);
        });
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| {
            app.preparing_artwork_path = None;
            app.palette_path = Some(path.clone());
            app.palette = prepared.palette;
            app.fullscreen_cover = Some(prepared.cover.clone());
            app.fullscreen_backdrop = prepared.background.clone();
            app.update_effects(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(view.read(cx).fullscreen);
        assert!(!view.read(cx).pending_fullscreen);
        assert!(view.read(cx).fullscreen_cover.is_some());
        assert!(view.read(cx).palette != super::artwork::Palette::default());
        assert!(window.find("fullscreen-bounce-cover").visible());
        // A pending HQ upgrade must preserve the ready fallback and palette.
        view.update(cx, |app, cx| {
            app.hq_thumbs.insert(5);
            let high = directory.path().join("hq.embed.jpg");
            app.preparing_artwork_path = Some(high.clone());
            app.playing_artwork_path = Some(high.clone());
            app.thumbs.insert(5, Some(high));
            assert!(!app.fullscreen_artwork_pending(cx));
            assert_eq!(app.palette_path.as_ref(), Some(&path));
            assert!(app.fullscreen_cover.is_some());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn precise_scroll_has_intermediate_positions_and_stops_at_the_input_target(
    cx: &mut TestAppContext,
) {
    cx.disable_accessibility();
    let (window, _) = fixture(cx, size(px(1200.), px(400.)));
    cx.update(|cx| cx.set_reduce_motion(false));
    let start = cx
        .update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let start = window.find(("track-row", 0_usize)).bounds().top();
            window.scroll(
                "track-rows",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-20.))),
                cx,
            );
            start
        })
        .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_nanos(8_333_333));
    let middle = cx
        .update_window(window, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            window.find(("track-row", 0_usize)).bounds().top()
        })
        .unwrap();
    assert!(
        middle < start && middle > start - px(20.),
        "start={start:?}, middle={middle:?}"
    );
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(30));
    let end = cx
        .update_window(window, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            window.find(("track-row", 0_usize)).bounds().top()
        })
        .unwrap();
    assert!((start - end - px(20.)).abs() < px(1.));
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.update_window(window, |_, window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find(("track-row", 0_usize)).bounds().top(), end);
    })
    .unwrap();
}

#[gpui_kit::test]
fn library_scrollbar_appears_on_scroll_and_drags_to_the_end(cx: &mut TestAppContext) {
    let (window, _) = fixture(cx, size(px(1200.), px(400.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "track-rows",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-80.))),
            cx,
        );
        window.render_frame(cx);
        let viewport = window.find("track-rows").bounds();
        let scale = window.scale_factor();
        let thumb = window
            .painted_quads()
            .into_iter()
            .find(|quad| {
                let width = quad.bounds.size.width.0 / scale;
                let left = quad.bounds.origin.x.0 / scale;
                (5. ..=8.).contains(&width)
                    && left > f32::from(viewport.right()) - 28.
                    && quad.bounds.size.height.0 / scale > 20.
            })
            .expect("a visible scrollbar thumb near the viewport edge");
        let start = gpui_kit::point(
            px((thumb.bounds.origin.x.0 + thumb.bounds.size.width.0 / 2.) / scale),
            px((thumb.bounds.origin.y.0 + thumb.bounds.size.height.0 / 2.) / scale),
        );
        window.drag(
            start,
            gpui_kit::point(start.x, viewport.bottom() - px(4.)),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find(("track-row", 6_usize)).visible());
        assert!(window
            .try_find(("track-row", 0_usize))
            .is_none_or(|row| !row.visible()));
    })
    .unwrap();
}

#[gpui_kit::test]
fn modal_backdrops_block_background_hover_and_scroll(cx: &mut TestAppContext) {
    use gpui_kit::{point, test::TestEventExt};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    for panel in [Panel::Navigation, Panel::Settings, Panel::NewPlaylist] {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let row = window.find(("track-row", 0_usize)).bounds();
            let target = point(row.right() - px(70.), row.center().y);
            let hovered_row = |window: &gpui_kit::Window| {
                let scale = window.scale_factor();
                window.painted_quads().iter().any(|quad| {
                    quad.background == gpui_kit::rgb(super::views::CARD).into()
                        && quad.bounds.origin.x.0 <= f32::from(target.x) * scale
                        && quad.bounds.origin.y.0 <= f32::from(target.y) * scale
                        && quad.bounds.right().0 >= f32::from(target.x) * scale
                        && quad.bounds.bottom().0 >= f32::from(target.y) * scale
                })
            };
            window.pointer_move(target, None, cx);
            window.render_frame(cx);
            assert!(
                hovered_row(window),
                "the uncovered row must actually respond to hover"
            );
            view.update(cx, |app, cx| app.open_panel(panel.clone(), window, cx));
            window.render_frame(cx);
            window.pointer_move(point(px(1150.), px(70.)), None, cx);
            window.pointer_move(target, None, cx);
            window.render_frame(cx);
            assert!(!hovered_row(window), "covered track must not highlight");
            let before = window.find(("track-row", 0_usize)).bounds();
            window.scroll(
                "track-rows",
                gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-200.))),
                cx,
            );
            assert_eq!(
                window.find(("track-row", 0_usize)).bounds(),
                before,
                "modal must block background scrolling"
            );
            let seek = window.find("playback-progress").bounds();
            window.pointer_move(
                point(seek.left() + seek.size.width * 0.75, seek.center().y),
                None,
                cx,
            );
            window.render_frame(cx);
            assert!(
                view.read(cx).seek_preview.is_none(),
                "covered seek rail must not preview"
            );
            window.hover("volume", cx);
            window.render_frame(cx);
            assert!(
                !view.read(cx).volume_hover,
                "covered volume control must not open"
            );
            view.update(cx, |app, cx| app.close_panel(window, cx));
            window.render_frame(cx);
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn modal_closing_animation_keeps_blocking_hover(cx: &mut TestAppContext) {
    use gpui_kit::{point, test::TestEventExt};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update(|cx| cx.set_reduce_motion(false));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |app, cx| app.open_panel(Panel::Navigation, window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |app, cx| app.close_panel(window, cx));
        window.render_frame(cx);
        assert!(view.read(cx).retained_modal.is_some());
        let seek = window.find("playback-progress").bounds();
        window.pointer_move(
            point(seek.left() + seek.size.width * 0.75, seek.center().y),
            None,
            cx,
        );
        window.render_frame(cx);
        assert!(
            view.read(cx).seek_preview.is_none(),
            "fading backdrop must still block hover"
        );
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(view.read(cx).retained_modal.is_none());
        let seek = window.find("playback-progress").bounds();
        window.pointer_move(point(px(1150.), px(70.)), None, cx);
        window.pointer_move(
            point(seek.left() + seek.size.width * 0.75, seek.center().y),
            None,
            cx,
        );
        window.render_frame(cx);
        assert!(
            view.read(cx).seek_preview.is_some(),
            "hover must return after the backdrop is gone"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn opening_modal_hides_an_existing_volume_hover_popup_immediately(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update(|cx| cx.set_reduce_motion(false));
    cx.update_window(window, |_, window, cx| window.hover("volume", cx))
        .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("volume-popover").is_some());
        assert!(view.read(cx).volume_hover);
        view.update(cx, |app, cx| app.open_panel(Panel::Navigation, window, cx));
        window.render_frame(cx);
        assert!(!view.read(cx).volume_hover);
        assert!(
            window.try_find("volume-popover").is_none(),
            "the old popup must not fade above the modal"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn exact_volume_focus_keeps_popup_open_outside_pointer_and_closes_after_blur(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{point, test::TestEventExt, Focusable};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    view.update(cx, |app, cx| {
        let snapshot = app.playback.read(cx).clone();
        app.command_port = Some(super::transport::CommandPort::with_test_handler(
            cx,
            move |_| {
                let snapshot = snapshot.clone();
                async { Ok(snapshot) }
            },
        ));
    });
    cx.update_window(window, |_, window, cx| window.hover("volume", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.activate_window();
        window.render_frame(cx);
        window.render_frame(cx);
        window.click("exact-volume-chip", cx);
        assert!(view
            .read(cx)
            .inputs
            .exact_volume
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
        window.pointer_move(point(px(700.), px(250.)), None, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(700));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window
            .try_find_by_label("Exact volume percentage")
            .is_some());
        window.press("secondary-a", cx);
        window.input("100", cx);
        assert_eq!(
            view.read(cx).inputs.exact_volume.read(cx).value().as_ref(),
            "100"
        );
        window.render_frame(cx);
        let input = view.read(cx).inputs.exact_volume.read(cx);
        let viewport = input.text_bounds().unwrap();
        let digits = input.range_to_bounds(&(0..3)).unwrap();
        assert!(
            digits.left() >= viewport.left(),
            "{digits:?} in {viewport:?}"
        );
        assert!(
            digits.right() <= viewport.right(),
            "{digits:?} in {viewport:?}"
        );
        view.update(cx, |app, cx| app.focus.focus(window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(700));
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            !view.read(cx).volume_hover,
            "pointer {:?}, focus {}, task {}",
            view.read(cx).volume_pointer_over,
            view.read(cx).volume_focus.contains_focused(window, cx),
            view.read(cx).volume_close_task.is_some()
        );
        assert!(window.try_find("volume-popover").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn fullscreen_paints_before_native_transition_and_preserves_existing_fullscreen(
    cx: &mut TestAppContext,
) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.toggle_fullscreen(window, cx));
        assert!(
            !window.is_fullscreen(),
            "native transition must wait for the new scene"
        );
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find("fullscreen-title").is_some());
        assert!(!window.is_fullscreen());
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert!(window.is_fullscreen());
        // Entering the player from an already fullscreen library must not exit
        // and re-enter the native Space.
        view.update(cx, |app, cx| {
            app.fullscreen = false;
            app.toggle_fullscreen(window, cx);
        });
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert!(window.is_fullscreen());
        // Cancelled requests cannot toggle the window after a newer request.
        view.update(cx, |app, cx| {
            app.toggle_fullscreen(window, cx);
            app.toggle_fullscreen(window, cx);
        });
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert!(window.is_fullscreen());
        assert!(view.read(cx).fullscreen);
    })
    .unwrap();
}

#[gpui_kit::test]
fn sidebar_resize_uses_the_kit_handle_at_the_panel_edge(cx: &mut TestAppContext) {
    use gpui_kit::{point, test::TestEventExt, MouseButton};
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let panel = window.find("sidebar-panel").bounds();
        let handle = window.find("sidebar-resize").bounds();
        assert!((handle.right() - panel.right()).abs() <= px(1.));
        let start = handle.center();
        window.pointer_down(start, MouseButton::Left, cx);
        window.pointer_move(start + point(px(10.), px(0.)), Some(MouseButton::Left), cx);
        let target = point(px(430.), start.y);
        window.pointer_move(target, Some(MouseButton::Left), cx);
        window.pointer_up(target, MouseButton::Left, cx);
        window.render_frame(cx);
        assert!((view.read(cx).preferences.sidebar_width - 430.).abs() <= 1.);
        assert!((window.find("sidebar-panel").bounds().size.width - px(430.)).abs() <= px(1.));
        assert!(!cx.has_active_drag());
    })
    .unwrap();
}

#[gpui_kit::test]
fn account_revocation_exits_fullscreen_without_waiting_for_remote_work(cx: &mut TestAppContext) {
    let (window, view) = fixture(cx, size(px(1200.), px(800.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |app, cx| app.toggle_fullscreen(window, cx));
        assert!(view.read(cx).fullscreen);
        view.update(cx, |app, cx| {
            app.event(crate::runtime::Event::AuthRevoked, cx)
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!view.read(cx).fullscreen);
        assert!(view.read(cx).user.is_none());
        assert!(window.try_find_by_label("Phone number").is_some());
    })
    .unwrap();
}
