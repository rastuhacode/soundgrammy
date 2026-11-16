//! Placement and structure of the original application's drawers and popovers.
use super::views::{glyph, heading, icon, muted, BORDER, CARD, MUTED, PRIMARY, SIDEBAR};
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    input::Input,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
};

impl PlayerApp {
    pub(super) fn navigation_drawer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let name = self
            .user
            .as_ref()
            .map(|u| {
                format!("{} {}", u.first_name, u.last_name.as_deref().unwrap_or(""))
                    .trim()
                    .to_owned()
            })
            .unwrap_or_default();
        let username = self
            .user
            .as_ref()
            .and_then(|u| u.username.as_ref())
            .map(|u| format!("@{u}"))
            .unwrap_or_else(|| "Telegram account".into());
        let status = if self.syncing {
            self.sync_progress
                .map(|(n, t)| format!("Syncing {n} of {t}"))
                .unwrap_or_else(|| "Syncing…".into())
        } else if self.online {
            "online".into()
        } else {
            "offline".into()
        };
        let sync_menu = view.clone();
        let syncing = self.syncing;
        let popup =
            div()
                .id("modal")
                .test_support()
                .role(Role::Group)
                .aria_label("Main navigation")
                .size_full()
                .bg(rgb(SIDEBAR))
                .border_r_1()
                .border_color(rgb(BORDER))
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .p_4()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(48.))
                                .flex_shrink_0()
                                .rounded_full()
                                .overflow_hidden()
                                .bg(rgba(0x2aabee26))
                                .flex()
                                .items_center()
                                .justify_center()
                                .when_some(self.avatar.clone(), |el, path| {
                                    el.child(
                                        img(path)
                                            .image_cache(&self.artwork_cache)
                                            .size_full()
                                            .object_fit(ObjectFit::Cover)
                                            .rounded_full(),
                                    )
                                })
                                .when(self.avatar.is_none(), |el| {
                                    el.child(
                                        div().text_xl().text_color(rgb(PRIMARY)).child(
                                            name.chars()
                                                .next()
                                                .map(|c| c.to_string())
                                                .unwrap_or_default(),
                                        ),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    heading(name.clone())
                                        .truncate()
                                        .hover_title(name, &self.hover_title_epoch),
                                )
                                .child(
                                    muted(username.clone())
                                        .hover_title(username, &self.hover_title_epoch)
                                        .text_xs()
                                        .text_color(rgb(PRIMARY))
                                        .truncate(),
                                ),
                        ),
                )
                .child(
                    div()
                        .py_2()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            gpui_kit::base::Button::new("nav-sync")
                                .accessibility_label("Sync Telegram music")
                                .hover_title(status.clone(), &self.hover_title_epoch)
                                .mx_2()
                                .px_2()
                                .min_h(px(48.))
                                .rounded(px(12.))
                                .flex()
                                .justify_start()
                                .items_center()
                                .gap_4()
                                .cursor_pointer()
                                .hover(|el| el.bg(rgb(0x203039)))
                                .child(
                                    glyph(IconName::RadioTower, 20.)
                                        .text_color(rgb(if self.online { PRIMARY } else { MUTED })),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Sync Telegram music"),
                                        )
                                        .child(muted(status).text_xs()),
                                )
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if !app.syncing {
                                        app.sync(false, cx)
                                    }
                                }))
                                .context_menu(move |menu, _, _| {
                                    let proxy = sync_menu.clone();
                                    let reindex = sync_menu.clone();
                                    menu.item(PopupMenuItem::new("Connection settings").on_click(
                                        move |_, window, cx| {
                                            proxy.update(cx, |app, cx| {
                                                app.panel = None;
                                                app.open_panel(Panel::Proxy, window, cx)
                                            })
                                        },
                                    ))
                                    .item(
                                        PopupMenuItem::new("Reindex entire library")
                                            .disabled(syncing)
                                            .on_click(move |_, _, cx| {
                                                reindex.update(cx, |app, cx| {
                                                    app.panel = None;
                                                    app.show_panel(Panel::Confirm(
                                                        Confirmation::FullReindex,
                                                    ));
                                                    cx.notify();
                                                })
                                            }),
                                    )
                                }),
                        )
                        .child(
                            gpui_kit::base::Button::new("nav-settings")
                                .accessibility_label("Settings")
                                .mx_2()
                                .px_2()
                                .min_h(px(48.))
                                .rounded(px(12.))
                                .flex()
                                .justify_start()
                                .items_center()
                                .gap_4()
                                .cursor_pointer()
                                .hover(|el| el.bg(rgb(0x203039)))
                                .child(glyph(IconName::Settings, 20.).text_color(rgb(MUTED)))
                                .child("Settings")
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.panel = None;
                                    app.panel_stack.clear();
                                    app.open_panel(Panel::Settings, window, cx)
                                })),
                        ),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .pt_2()
                        .pb_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            gpui_kit::base::Button::new("nav-logout")
                                .accessibility_label("Log out")
                                .mx_2()
                                .px_2()
                                .min_h(px(48.))
                                .rounded(px(12.))
                                .flex()
                                .justify_start()
                                .items_center()
                                .gap_4()
                                .text_color(rgb(0xff656a))
                                .cursor_pointer()
                                .hover(|el| el.bg(rgba(0xff656a1a)))
                                .child(glyph(IconName::LogOut, 20.))
                                .child("Log out")
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.panel = None;
                                    app.panel_stack.clear();
                                    app.show_panel(Panel::Confirm(Confirmation::Logout));
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .px_5()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .text_xs()
                                .text_color(rgb(MUTED))
                                .child(div().font_weight(FontWeight::MEDIUM).child("SoundGrammy"))
                                .child(
                                    gpui_kit::base::Link::new("nav-source")
                                        .accessibility_label("Version: 3.5.0")
                                        .href("https://github.com/rastuhacode/soundgrammy/releases")
                                        .cursor_pointer()
                                        .child(format!("Version: {}", env!("CARGO_PKG_VERSION"))),
                                ),
                        ),
                )
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .into_any_element();
        self.modal_surface(popup, true, window, cx)
    }

    pub(super) fn queue_popover(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let queue = self.snapshot.player.as_ref().map(|p| &p.queue);
        let position = queue
            .filter(|q| !q.tracks.is_empty())
            .map(|q| format!("{} of {}", q.cursor + 1, q.tracks.len()))
            .unwrap_or_default();
        let source = queue
            .and_then(|q| q.source.as_ref())
            .map(|s| format!("From {}", s.name))
            .unwrap_or_else(|| "Custom queue".into());
        let compact = window.viewport_size().width < px(768.);
        let content = self.queue_view(compact, cx);
        div()
            .id("modal")
            .test_support()
            .aria_label("Queue")
            .w(px(320.))
            .h(px(450.))
            .max_w_full()
            .max_h(px(
                (f32::from(window.viewport_size().height) - 150.).max(160.)
            ))
            .rounded(px(16.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(CARD))
            .shadow_lg()
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px_3()
                    .py(px(10.))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(heading("Queue").text_sm().line_height(px(20.)))
                                    .child(
                                        muted(position)
                                            .text_size(px(11.))
                                            .line_height(px(16.))
                                            .font_family(mono_font()),
                                    ),
                            )
                            .child(
                                muted(source.clone())
                                    .text_xs()
                                    .line_height(px(16.))
                                    .truncate()
                                    .hover_title(source, &self.hover_title_epoch),
                            ),
                    )
                    .child(
                        icon("queue-actions", IconName::Ellipsis)
                            .accessibility_label("Queue actions")
                            .dropdown_menu(move |menu, _, _| {
                                let save = view.clone();
                                let next = view.clone();
                                let clear = view.clone();
                                menu.item(PopupMenuItem::new("Save as playlist").on_click(
                                    move |_, window, cx| {
                                        save.update(cx, |app, cx| {
                                            app.open_panel(Panel::SaveQueue, window, cx)
                                        })
                                    },
                                ))
                                .separator()
                                .item(PopupMenuItem::new("Clear up next").on_click(
                                    move |_, _, cx| {
                                        next.update(cx, |app, cx| {
                                            app.command(Command::ClearUpNext, cx)
                                        })
                                    },
                                ))
                                .item(
                                    PopupMenuItem::new("Clear").on_click(move |_, _, cx| {
                                        clear.update(cx, |app, cx| app.command(Command::Clear, cx))
                                    }),
                                )
                            }),
                    )
                    .child(
                        icon("close-panel", IconName::X)
                            .accessibility_label("Close queue")
                            .on_click(
                                cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                            ),
                    ),
            )
            .child(div().min_h_0().flex_1().child(content))
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .into_any_element()
    }

    pub(super) fn filters_popover(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("modal")
            .test_support()
            .aria_label("Playlists filters")
            .w(px(224.))
            .p_2()
            .rounded(px(16.))
            .bg(rgb(CARD))
            .border_1()
            .border_color(rgb(BORDER))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Input::new(&self.inputs.playlist_search)
                    .prefix(glyph(IconName::Search, 16.))
                    .aria_label("Search playlists"),
            )
            .child(div().mt_2().mb_2().border_b_1().border_color(rgb(BORDER)))
            .children(
                [
                    ("recency", "Recency"),
                    ("custom", "Custom Order"),
                    ("alphabetical", "Alphabetical"),
                ]
                .into_iter()
                .map(|(key, label)| {
                    let selected = self.preferences.playlist_sort == key;
                    gpui_kit::base::Radio::new(SharedString::from(format!("playlist-sort-{key}")))
                        .key_context("PlaylistSort")
                        .checked(selected)
                        .accessibility_label(label)
                        .track_focus(
                            &self.playlist_sort_focus[match key {
                                "recency" => 0,
                                "custom" => 1,
                                _ => 2,
                            }],
                        )
                        .on_key_down(cx.listener(move |app, event: &KeyDownEvent, window, cx| {
                            let index = match key {
                                "recency" => 0,
                                "custom" => 1,
                                _ => 2,
                            };
                            let next = match event.keystroke.key.as_str() {
                                "up" | "left" => (index + 2) % 3,
                                "down" | "right" => (index + 1) % 3,
                                "home" => 0,
                                "end" => 2,
                                _ => return,
                            };
                            app.preferences.playlist_sort =
                                ["recency", "custom", "alphabetical"][next].into();
                            app.playlist_sort_focus[next].focus(window, cx);
                            app.save_preferences();
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .h(px(28.))
                        .px_2()
                        .rounded(px(12.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .cursor_pointer()
                        .when(selected, |el| el.bg(rgb(0x203039)))
                        .hover(|el| el.bg(rgb(0x272727)))
                        .child(label)
                        .when(selected, |el| el.child(glyph(IconName::Check, 16.)))
                        .on_change({
                            let change = cx.listener(move |app, _: &ClickEvent, _, cx| {
                                app.preferences.playlist_sort = key.into();
                                app.save_preferences();
                                cx.notify();
                            });
                            move |_, event, window, cx| change(event, window, cx)
                        })
                }),
            )
            .child(div().my_2().border_b_1().border_color(rgb(BORDER)))
            .child(
                super::settings::check(
                    "reverse-playlists",
                    "Reverse",
                    self.preferences.reversed && self.preferences.playlist_sort != "custom",
                    cx.listener(|app, _, _, cx| {
                        if app.preferences.playlist_sort != "custom" {
                            app.preferences.reversed = !app.preferences.reversed;
                            app.save_preferences();
                            cx.notify();
                        }
                    }),
                )
                .disabled(self.preferences.playlist_sort == "custom")
                .px_2()
                .py_1()
                .when(self.preferences.playlist_sort == "custom", |el| {
                    el.opacity(0.5)
                }),
            )
            .when(!self.preferences.hidden.is_empty(), |el| {
                el.child(div().my_2().border_b_1().border_color(rgb(BORDER)))
                    .child(muted("Hidden Playlists").text_xs().px_2())
                    .children(
                        [
                            ("liked", "Liked"),
                            ("popular", "Popular"),
                            ("recent", "Recent"),
                        ]
                        .into_iter()
                        .filter(|(id, _)| self.preferences.hidden.contains(*id))
                        .map(|(id, label)| {
                            gpui_kit::base::Button::new(SharedString::from(format!("unhide-{id}")))
                                .accessibility_label(label)
                                .h(px(32.))
                                .px_2()
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .child(label)
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.preferences.hidden.remove(id);
                                    app.save_preferences();
                                    cx.notify();
                                }))
                        }),
                    )
            })
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .into_any_element()
    }

    pub(super) fn compact_player_view(
        &mut self,
        motion: gpui_kit::base::motion::PresenceSample,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let track = self
            .snapshot
            .player
            .as_ref()
            .and_then(|p| p.queue.current())
            .cloned();
        if let Some(t) = &track {
            self.high_quality_thumbnail(t.id, cx);
        }
        let source = self
            .snapshot
            .player
            .as_ref()
            .and_then(|p| p.queue.source.as_ref())
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Queue".into());
        let width = f32::from(window.viewport_size().width);
        let height = f32::from(window.viewport_size().height);
        let top = if cfg!(target_os = "macos") && !window.is_fullscreen() {
            32.
        } else {
            0.
        };
        let art = (width - 56.).min(height - top - 340.).max(64.);
        let exiting = motion.phase == gpui_kit::base::motion::PresencePhase::Exiting;
        let slide = px(height * (1. - motion.progress));
        let surface =
            div()
                .id("compact-player")
                .test_support()
                .role(Role::Dialog)
                .aria_label("Now playing")
                .absolute()
                .inset_0()
                .top(px(top) + slide)
                .bottom(-slide)
                .bg(rgb(CARD))
                .flex()
                .flex_col()
                .occlude()
                .child(
                    div()
                        .px_5()
                        .pt(px(if cfg!(target_os = "android") {
                            48.
                        } else {
                            12.
                        }))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            icon("close-compact-player", IconName::ChevronDown)
                                .w(px(44.))
                                .h(px(44.))
                                .accessibility_label("Close player")
                                .on_click(
                                    cx.listener(|app, _, window, cx| app.close_panel(window, cx)),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_center()
                                .child(
                                    muted("NOW PLAYING")
                                        .text_size(px(10.))
                                        .line_height(px(14.))
                                        .font_weight(FontWeight::SEMIBOLD),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .font_weight(FontWeight::MEDIUM)
                                        .id("compact-player-source")
                                        .hover_title(source.clone(), &self.hover_title_epoch)
                                        .child(source),
                                ),
                        )
                        .child(self.queue_button("compact-player-queue", 44., window, cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .px_7()
                        .py_5()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .size(px(art))
                                .rounded(px(20.))
                                .border_1()
                                .border_color(rgb(BORDER))
                                .bg(rgb(0x272727))
                                .shadow_lg()
                                .when_some(
                                    track
                                        .as_ref()
                                        .and_then(|t| self.thumbs.get(&t.id).cloned().flatten()),
                                    |el, path| {
                                        el.child(
                                            img(path)
                                                .image_cache(&self.artwork_cache)
                                                .size_full()
                                                .object_fit(ObjectFit::Cover)
                                                .rounded(px(20.)),
                                        )
                                    },
                                ),
                        ),
                )
                .child(
                    div()
                        .px_4()
                        .pb(px(if cfg!(target_os = "android") {
                            48.
                        } else {
                            24.
                        }))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .when_some(track.as_ref(), |el, t| {
                                    el.child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                heading(title(t))
                                                    .hover_title(title(t), &self.hover_title_epoch)
                                                    .text_xl(),
                                            )
                                            .child(
                                                muted(artist(t)).truncate().hover_title(
                                                    artist(t),
                                                    &self.hover_title_epoch,
                                                ),
                                            ),
                                    )
                                })
                                .child(self.like_current("compact-player-like", 44., 24., cx)),
                        )
                        .child(div().h(px(28.)).flex().items_center().child(
                            self.playback_progress("sheet", track.is_none() || exiting, window, cx),
                        ))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .font_family(mono_font())
                                .text_xs()
                                .text_color(rgb(MUTED))
                                .child(time(self.snapshot.current_time_seconds))
                                .child(time(self.snapshot.duration_seconds)),
                        )
                        .child(
                            div()
                                .mt_3()
                                .flex()
                                .justify_center()
                                .child(self.playback_controls(false, true, width, cx)),
                        ),
                )
                .when(exiting, |el| el.child(super::motion::exit_shield()))
                .into_any_element();
        if exiting {
            return div().absolute().inset_0().child(surface).into_any_element();
        }
        gpui_kit::base::Sheet::new(cx)
            .focus_handle(self.modal_focus.clone())
            .overlay_closable(false)
            .surface(surface)
            .request_close({
                let view = cx.entity();
                move |window, cx| view.update(cx, |app, cx| app.close_panel(window, cx))
            })
            .into_any_element()
    }
}
