//! Content layouts shared with the original playlist and metadata dialogs.
use super::views::{button, glyph, heading, icon, muted, BORDER, CARD, MUTED};
use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::base::{Dialog, DialogPopup};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    Disableable,
};

fn metadata_rows(rows: Vec<(&'static str, String)>) -> Div {
    div()
        .p_3()
        .border_1()
        .border_color(rgb(BORDER))
        .rounded(px(12.))
        .flex()
        .flex_col()
        .gap_2()
        .children(rows.into_iter().map(|(label, value)| {
            div()
                .flex()
                .gap_2()
                .text_sm()
                .line_height(px(20.))
                .child(muted(label).w(px(112.)).flex_shrink_0())
                .child(div().flex_1().min_w_0().child(value))
        }))
}
fn bytes(value: Option<i64>) -> String {
    match value {
        None => "—".into(),
        Some(n) if n <= 0 => "—".into(),
        Some(n) if n < 1024 => format!("{n} B"),
        Some(n) if n < 1024 * 1024 => format!("{:.1} KB", n as f64 / 1024.),
        Some(n) => format!("{:.1} MB", n as f64 / 1048576.),
    }
}
fn played_at(value: Option<i64>) -> String {
    value
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%-m/%-d/%Y, %-I:%M:%S %p")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}
fn listened(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    if seconds >= 3600 {
        format!(
            "{}h {}m {}s",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}
impl PlayerApp {
    pub(super) fn modal_surface(
        &self,
        popup: AnyElement,
        docked: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let inset = if cfg!(target_os = "macos") && !window.is_fullscreen() {
            32.
        } else {
            0.
        };
        let progress = self.modal_progress;
        let active = self.panel.as_ref().is_some_and(|panel| {
            !matches!(
                panel,
                Panel::Queue | Panel::PlaylistFilters | Panel::CompactPlayer
            )
        });
        let backdrop = div()
            .absolute()
            .top(px(inset))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(rgba(0x00000066))
            .opacity(progress)
            // Dimming/blur only paints pixels. Occlusion excludes underlying
            // hitboxes from hover, pointer and scroll targeting as well.
            .occlude()
            // This hitbox also covers Dialog's outer dismissal listener, so
            // handle outside presses here while retaining its focus handling.
            .on_any_mouse_down(cx.listener(move |app, event: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                if active && event.button == MouseButton::Left {
                    app.close_panel(window, cx);
                }
            }))
            .on_scroll(|_, _, cx| cx.stop_propagation());
        let surface = div()
            .relative()
            .occlude()
            .max_w_full()
            .when(docked, |el| el.size_full())
            .opacity(if docked { 1. } else { progress })
            .when(!docked, |el| el.top(px(4. * (1. - progress))))
            .child(popup)
            .when(!active, |el| el.child(super::motion::exit_shield()));
        let position = |el: Div| {
            el.when(docked, |el| {
                el.absolute()
                    .left(px(-320. * (1. - progress)))
                    .top(px(inset))
                    .bottom_0()
                    .w(px(320.))
            })
        };
        if !active {
            return deferred(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_4()
                    .pt(px(inset + 16.))
                    .child(backdrop)
                    .child(position(surface)),
            )
            .with_priority(10)
            .into_any_element();
        }
        let close = cx.listener(|app, _: &ClickEvent, window, cx| app.close_panel(window, cx));
        Dialog::new(cx)
            .focus_handle(self.modal_focus.clone())
            .dismiss_below_y(px(inset))
            .on_ok(|_, _, _| false)
            .on_close(close)
            .backdrop(backdrop)
            .popup(
                DialogPopup::new()
                    .when(docked, |el| {
                        el.absolute()
                            .left(px(-320. * (1. - progress)))
                            .top(px(inset))
                            .bottom_0()
                            .w(px(320.))
                    })
                    .child(surface),
            )
            .p_4()
            .pt(px(inset + 16.))
            .into_any_element()
    }
    pub(super) fn detail_dialog(
        &self,
        text: (&'static str, &str),
        body: AnyElement,
        footer: Option<AnyElement>,
        dimensions: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (title, description) = text;
        let (width, max_height) = dimensions;
        let popup = div()
            .id("modal")
            .test_support()
            .role(Role::Group)
            .aria_label(title)
            .relative()
            .w(px(width))
            .max_w_full()
            .max_h(px(max_height))
            .flex()
            .flex_col()
            .rounded(px(16.))
            .border_1()
            .border_color(rgb(BORDER))
            .bg(rgb(CARD))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .p_4()
                    .pb_0()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(heading(title).line_height(px(20.)).pr_6())
                    .child(muted(description.to_owned())),
            )
            .child(
                div()
                    .id("panel-scroll")
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .when_some(self.error.clone(), |el, error| {
                        el.child(
                            div()
                                .mb_3()
                                .text_sm()
                                .text_color(rgb(0xff656a))
                                .child(error),
                        )
                    })
                    .child(body),
            )
            .when_some(footer, |el, footer| {
                el.child(
                    div()
                        .p_4()
                        .flex_shrink_0()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(0x242424))
                        .rounded_b(px(16.))
                        .child(footer),
                )
            })
            .child(
                icon("close-panel", IconName::X)
                    .absolute()
                    .top(px(8.))
                    .right(px(8.))
                    .accessibility_label("Close")
                    .on_click(cx.listener(|app, _, window, cx| app.close_panel(window, cx))),
            )
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .into_any_element();
        self.modal_surface(popup, false, window, cx)
    }
    pub(super) fn playlist_dialog(
        &self,
        panel: &Panel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let creating = matches!(panel, Panel::NewPlaylist);
        let importing = creating && self.new_playlist_import;
        let title = if importing {
            "Import playlist"
        } else if creating {
            "Create playlist"
        } else if matches!(panel, Panel::SaveQueue) {
            "Save queue as playlist"
        } else {
            "Rename playlist"
        };
        let description = if importing {
            "Import playlist from file. Keep in mind that it is not cross-user operation."
        } else if creating {
            "Create a playlist from scratch."
        } else if matches!(panel, Panel::SaveQueue) {
            "Create a new custom playlist from the current queue. The queue itself is unchanged."
        } else {
            "Choose a new name for this playlist."
        };
        let body = div()
            .flex()
            .flex_col()
            .gap_4()
            .when(creating, |el| {
                el.child(
                    gpui_kit::base::Tabs::new("playlist-tabs")
                        .key_context("PlaylistTabs")
                        .p(px(3.))
                        .h(px(32.))
                        .w_full()
                        .rounded(px(12.))
                        .bg(rgb(0x272727))
                        .flex()
                        .children([(false, "New"), (true, "Import")].into_iter().map(
                            |(tab, label)| {
                                gpui_kit::base::Tab::new(label)
                                    .accessibility_label(label)
                                    .selected(importing == tab)
                                    .track_focus(&self.playlist_tab_focus[usize::from(tab)])
                                    .set_position(usize::from(tab) + 1, 2)
                                    .on_key_down(cx.listener(
                                        move |app, event: &KeyDownEvent, window, cx| {
                                            let next = match event.keystroke.key.as_str() {
                                                "left" | "right" => !tab,
                                                "home" => false,
                                                "end" => true,
                                                _ => return,
                                            };
                                            app.new_playlist_import = next;
                                            app.playlist_tab_focus[usize::from(next)]
                                                .focus(window, cx);
                                            cx.stop_propagation();
                                            cx.notify();
                                        },
                                    ))
                                    .child(label)
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(10.))
                                    .cursor_pointer()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(if importing == tab {
                                        0xf2f2f2
                                    } else {
                                        MUTED
                                    }))
                                    .when(importing == tab, |el| {
                                        el.bg(rgb(0x2c2c2c)).border_1().border_color(rgb(BORDER))
                                    })
                                    .on_click(cx.listener(move |app, _, window, cx| {
                                        app.new_playlist_import = tab;
                                        if !tab {
                                            app.inputs.name.update(cx, |s, cx| s.focus(window, cx));
                                        }
                                        cx.notify();
                                    }))
                            },
                        )),
                )
            })
            .when(!importing, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .line_height(px(20.))
                                .child("Name"),
                        )
                        .child(Input::new(&self.inputs.name).aria_label("Name")),
                )
            })
            .when(matches!(panel, Panel::SaveQueue), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().font_weight(FontWeight::MEDIUM).child("Include"))
                        .children(
                            [
                                (
                                    QueueSaveScope::Full,
                                    "Full queue",
                                    "History, now playing, and up next",
                                ),
                                (
                                    QueueSaveScope::FromHere,
                                    "From here",
                                    "Now playing and everything after",
                                ),
                                (
                                    QueueSaveScope::UpNext,
                                    "Up next only",
                                    "Only tracks after the current one",
                                ),
                            ]
                            .into_iter()
                            .map(|(scope, label, description)| {
                                let selected = self.queue_save_scope == scope;
                                gpui_kit::base::Radio::new(label)
                                    .checked(selected)
                                    .accessibility_label(label)
                                    .track_focus(&self.queue_scope_focus[scope as usize])
                                    .tab_stop(selected)
                                    .on_key_down(cx.listener(
                                        move |app, event: &KeyDownEvent, window, cx| {
                                            if matches!(
                                                event.keystroke.key.as_str(),
                                                "up" | "left" | "down" | "right"
                                            ) {
                                                let next = (scope as usize
                                                    + if matches!(
                                                        event.keystroke.key.as_str(),
                                                        "down" | "right"
                                                    ) {
                                                        1
                                                    } else {
                                                        2
                                                    })
                                                    % 3;
                                                app.queue_save_scope = [
                                                    QueueSaveScope::Full,
                                                    QueueSaveScope::FromHere,
                                                    QueueSaveScope::UpNext,
                                                ][next];
                                                app.queue_scope_focus[next].focus(window, cx);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .px_3()
                                    .py_2()
                                    .border_1()
                                    .border_color(rgb(if selected {
                                        super::views::PRIMARY
                                    } else {
                                        BORDER
                                    }))
                                    .rounded(px(10.))
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .cursor_pointer()
                                    .when(selected, |el| el.bg(rgba(0x2aabee0d)))
                                    .child(
                                        div()
                                            .mt(px(2.))
                                            .size(px(16.))
                                            .rounded_full()
                                            .border_1()
                                            .border_color(rgb(if selected {
                                                super::views::PRIMARY
                                            } else {
                                                BORDER
                                            }))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(selected, |el| {
                                                el.child(
                                                    div()
                                                        .size(px(8.))
                                                        .rounded_full()
                                                        .bg(rgb(super::views::PRIMARY)),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(
                                                div().font_weight(FontWeight::MEDIUM).child(label),
                                            )
                                            .child(
                                                muted(description).text_xs().line_height(px(16.)),
                                            ),
                                    )
                                    .on_change({
                                        let change =
                                            cx.listener(move |app, _: &ClickEvent, _, cx| {
                                                app.queue_save_scope = scope;
                                                cx.notify();
                                            });
                                        move |_, event, window, cx| change(event, window, cx)
                                    })
                            }),
                        ),
                )
            })
            .when(importing, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .line_height(px(20.))
                                .child("Playlist file"),
                        )
                        .child(
                            button("import-playlist", "Choose JSON file")
                                .self_start()
                                .outline()
                                .icon(glyph(IconName::FileUp, 16.))
                                .on_click(cx.listener(|app, _, _, cx| app.import_playlist(cx))),
                        )
                        .child(
                            muted(
                                "Select a `.soundgrammy.json` export from this Telegram account.",
                            )
                            .text_xs()
                            .line_height(px(16.)),
                        ),
                )
            })
            .into_any_element();
        let footer = div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                button("cancel-playlist", "Cancel")
                    .outline()
                    .on_click(cx.listener(|app, _, window, cx| app.close_panel(window, cx))),
            )
            .child(
                Button::new("save-playlist")
                    .primary()
                    .label(if creating {
                        "Create playlist"
                    } else if matches!(panel, Panel::SaveQueue) {
                        "Save playlist"
                    } else {
                        "Save"
                    })
                    .disabled(
                        self.busy > 0
                            || importing
                            || self.inputs.name.read(cx).value().trim().is_empty(),
                    )
                    .on_click(cx.listener(|app, _, _, cx| app.save_playlist(cx))),
            )
            .into_any_element();
        self.detail_dialog(
            (title, description),
            body,
            Some(footer),
            (448., f32::from(window.viewport_size().height) * 0.9),
            window,
            cx,
        )
    }
    pub(super) fn track_info_dialog(
        &self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let track = self.library.tracks.iter().find(|t| t.id == id);
        let body = div()
            .flex()
            .flex_col()
            .gap_5()
            .when_some(track, |el, t| {
                el.child(
                    div()
                        .flex()
                        .items_start()
                        .gap_4()
                        .child(
                            div()
                                .size(px(96.))
                                .flex_shrink_0()
                                .rounded(px(12.))
                                .border_1()
                                .border_color(rgb(BORDER))
                                .when_some(self.thumbs.get(&id).cloned().flatten(), |el, path| {
                                    el.child(
                                        img(path)
                                            .size_full()
                                            .object_fit(ObjectFit::Cover)
                                            .rounded(px(12.)),
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
                                .pt_1()
                                .child(heading(super::model::title(t)))
                                .child(muted(super::model::artist(t)))
                                .child(
                                    muted(super::model::time(t.duration.unwrap_or(0) as f64))
                                        .font_family(mono_font())
                                        .text_xs()
                                        .mt_2(),
                                ),
                        ),
                )
            })
            .when_some(
                self.library
                    .stats
                    .get(&id)
                    .filter(|_| self.statistics_enabled),
                |el, s| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(heading("Listening").text_sm())
                            .child(metadata_rows(vec![
                                ("Likeness", format!("{:.2}", s.likeness)),
                                ("Qualified", s.qualified_plays.to_string()),
                                ("Completes", s.completes.to_string()),
                                ("Early skips", s.early_skips.to_string()),
                                ("Starts", s.starts.to_string()),
                                ("Listened", listened(s.total_listened_ms)),
                                ("First played", played_at(s.first_played_at_ms)),
                                ("Last played", played_at(s.last_played_at_ms)),
                            ])),
                    )
                },
            )
            .when_some(track, |el, t| {
                let meta = self
                    .metadata
                    .as_ref()
                    .filter(|(key, _)| *key == id)
                    .map(|(_, m)| m);
                let mut rows = vec![
                    ("Title", super::model::title(t)),
                    ("Artist", super::model::artist(t)),
                    (
                        "Duration",
                        super::model::time(t.duration.unwrap_or(0) as f64),
                    ),
                    (
                        "MIME type",
                        t.mime_type.clone().unwrap_or_else(|| "—".into()),
                    ),
                    ("File size", bytes(t.file_size)),
                ];
                if let Some(m) = meta {
                    rows.extend([
                        ("Source", m.track.source.clone()),
                        ("File ID", m.track.file_id.clone()),
                        ("Unique ID", m.track.file_unique_id.clone()),
                    ]);
                }
                rows.push(("Added", t.created_at.clone()));
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(heading("Track").text_sm())
                        .child(metadata_rows(rows)),
                )
            })
            .when_some(
                self.metadata.as_ref().filter(|(key, _)| *key == id),
                |el, (_, m)| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(heading("Document").text_sm())
                            .child(metadata_rows(vec![
                                ("Document ID", m.document.id.clone()),
                                ("DC ID", m.document.dc_id.to_string()),
                                (
                                    "MIME type",
                                    m.document.mime_type.clone().unwrap_or_else(|| "—".into()),
                                ),
                                ("Size", bytes(m.document.size)),
                                (
                                    "Thumbnail",
                                    if m.document.has_remote_thumb {
                                        "Remote thumb on Telegram"
                                    } else {
                                        "None"
                                    }
                                    .into(),
                                ),
                            ])),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(heading("Attributes").text_sm())
                            .children(m.document.attributes.iter().map(|attribute| {
                                let object = attribute.as_object();
                                let name = object
                                    .and_then(|a| a.get("type"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Attribute");
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .p_3()
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .rounded(px(12.))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(name.to_owned()),
                                    )
                                    .when_some(object, |el, object| {
                                        el.children(
                                            object
                                                .iter()
                                                .filter(|(k, _)| k.as_str() != "type")
                                                .map(|(k, v)| {
                                                    div()
                                                        .flex()
                                                        .gap_2()
                                                        .text_sm()
                                                        .child(muted(k.clone()).w(px(112.)))
                                                        .child(
                                                            v.as_str()
                                                                .map(str::to_owned)
                                                                .unwrap_or_else(|| v.to_string()),
                                                        )
                                                }),
                                        )
                                    })
                            })),
                    )
                },
            )
            .into_any_element();
        self.detail_dialog(
            (
                "Track info",
                "Metadata from your library and Telegram document attributes.",
            ),
            body,
            None,
            (
                512.,
                ((f32::from(window.viewport_size().height)
                    - if cfg!(target_os = "macos") && !window.is_fullscreen() {
                        32.
                    } else {
                        0.
                    })
                    * 0.85)
                    .min(720.),
            ),
            window,
            cx,
        )
    }
}

impl PlayerApp {
    pub(super) fn logs_dialog(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let body = div()
            .mx(px(-16.))
            .my(px(-16.))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_y_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        muted(format!(
                            "{} {}",
                            self.logs.entries().len(),
                            if self.logs.entries().len() == 1 {
                                "entry"
                            } else {
                                "entries"
                            }
                        ))
                        .font_family(mono_font())
                        .text_xs(),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                button("copy-logs", "Copy all")
                                    .outline()
                                    .icon(glyph(IconName::Copy, 16.))
                                    .disabled(self.logs.entries().is_empty())
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            app.logs.join("\n\n"),
                                        ))
                                    })),
                            )
                            .child(
                                button("clear-logs", "Clear")
                                    .danger()
                                    .icon(glyph(IconName::Trash, 16.))
                                    .disabled(self.logs.entries().is_empty())
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.logs.clear();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .p_4()
                    .bg(rgba(0x27272733))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when(self.logs.entries().is_empty(), |el| {
                        el.child(
                            div()
                                .h(px(192.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(muted("No diagnostic logs recorded.")),
                        )
                    })
                    .children(self.logs.entries().iter().enumerate().rev().map(
                        |(index, entry)| {
                            let copied = entry.clone();
                            div()
                                .relative()
                                .p_3()
                                .pr_10()
                                .rounded(px(12.))
                                .border_1()
                                .border_color(rgb(BORDER))
                                .bg(rgb(0x141414))
                                .child(
                                    div()
                                        .font_family(mono_font())
                                        .text_size(px(11.))
                                        .line_height(px(18.))
                                        .child(entry.clone()),
                                )
                                .child(
                                    icon(("copy-log", index), IconName::Copy)
                                        .absolute()
                                        .right(px(8.))
                                        .top(px(8.))
                                        .w(px(24.))
                                        .h(px(24.))
                                        .accessibility_label("Copy entry")
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                copied.clone(),
                                            ))
                                        }),
                                )
                        },
                    )),
            )
            .into_any_element();
        self.detail_dialog(
            (
                "Diagnostic logs",
                "Logs stay on this device until they are cleared. Review them before sharing.",
            ),
            body,
            None,
            (
                768.,
                (f32::from(window.viewport_size().height)
                    - if cfg!(target_os = "macos") && !window.is_fullscreen() {
                        32.
                    } else {
                        0.
                    })
                    * 0.85,
            ),
            window,
            cx,
        )
    }
}

impl PlayerApp {
    pub(super) fn download_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let result = self.download_results.front();
        let description = result
            .map(|r| {
                format!(
                    "{} downloaded, {} failed.",
                    r.succeeded.len(),
                    r.failed.len()
                )
            })
            .unwrap_or_default();
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .when_some(result, |el, r| {
                el.children(r.warnings.iter().map(|warning| muted(warning.clone())))
                    .children(
                        [
                            (
                                "Downloaded",
                                r.succeeded
                                    .iter()
                                    .map(|t| {
                                        (
                                            format!(
                                                "{} — {}",
                                                t.performer.as_deref().unwrap_or("Unknown artist"),
                                                t.title.as_deref().unwrap_or("Unknown title")
                                            ),
                                            None,
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                            (
                                "Not downloaded",
                                r.failed
                                    .iter()
                                    .map(|t| {
                                        (
                                            format!(
                                                "{} — {}",
                                                t.performer.as_deref().unwrap_or("Unknown artist"),
                                                t.title.as_deref().unwrap_or("Unknown title")
                                            ),
                                            Some(t.error.clone()),
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                        ]
                        .into_iter()
                        .filter(|(_, items)| !items.is_empty())
                        .map(|(label, items)| {
                            let open = self.download_sections.contains(label);
                            div()
                                .border_1()
                                .border_color(rgb(BORDER))
                                .rounded(px(10.))
                                .flex()
                                .flex_col()
                                .child(
                                    gpui_kit::base::Button::new(label)
                                        .accessibility_label(label)
                                        .aria_expanded(open)
                                        .px_3()
                                        .py_2()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .cursor_pointer()
                                        .child(format!("{label} ({})", items.len()))
                                        .child(glyph(
                                            if open {
                                                IconName::ChevronUp
                                            } else {
                                                IconName::ChevronDown
                                            },
                                            16.,
                                        ))
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            if !app.download_sections.remove(label) {
                                                app.download_sections.insert(label);
                                            }
                                            cx.notify();
                                        })),
                                )
                                .when(open, |el| {
                                    el.child(
                                        div()
                                            .id(SharedString::from(format!("{label}-items")))
                                            .max_h(px(160.))
                                            .overflow_y_scroll()
                                            .border_t_1()
                                            .border_color(rgb(BORDER))
                                            .px_3()
                                            .py_2()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .children(items.into_iter().map(|(title, error)| {
                                                div().child(muted(title)).when_some(
                                                    error,
                                                    |el, error| {
                                                        el.child(
                                                            muted(error)
                                                                .test_support()
                                                                .text_xs()
                                                                .line_height(px(16.)),
                                                        )
                                                    },
                                                )
                                            })),
                                    )
                                })
                        }),
                    )
            })
            .into_any_element();
        let footer = div()
            .flex()
            .justify_end()
            .gap_2()
            .when_some(result.and_then(|r| r.folder_path.clone()), |el, path| {
                el.child(
                    button("open-download-folder", "Open folder")
                        .on_click(move |_, _, cx| cx.reveal_path(std::path::Path::new(&path))),
                )
            })
            .child(
                Button::new("dismiss-download-result")
                    .primary()
                    .label("OK")
                    .on_click(cx.listener(|app, _, window, cx| app.close_panel(window, cx))),
            )
            .into_any_element();
        self.detail_dialog(
            ("Download finished", &description),
            body,
            Some(footer),
            (448., f32::from(window.viewport_size().height) * 0.9),
            window,
            cx,
        )
    }
}

impl PlayerApp {
    pub(super) fn show_about(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        #[cfg(all(target_os = "macos", not(test)))]
        if let Some(main) = objc2::MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(main)
                .orderFrontStandardAboutPanel(None);
            return;
        }
        self.open_panel(Panel::About, _window, _cx);
    }
}
