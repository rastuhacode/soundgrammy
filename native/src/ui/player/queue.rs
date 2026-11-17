use super::super::views::*;
use super::super::*;
use gpui_kit::assets::IconName;

impl PlayerApp {
    pub(in crate::ui) fn queue_view(
        &mut self,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let session = self.playback.read(cx).player.as_ref();
        let queue = session.map(|s| s.queue.clone()).unwrap_or_default();
        let revision = session.map(|s| s.revision).unwrap_or(0);
        let next = queue_items(
            queue.tracks.len(),
            queue.cursor,
            self.preferences.history_open,
            self.preferences.up_next_open,
        );
        if self.queue_items != next {
            self.queue_list.reset(next.len());
            self.queue_items = next;
        }
        let items = Arc::new(self.queue_items.clone());
        div()
            .flex()
            .flex_col()
            .min_h_0()
            .when(queue.tracks.is_empty(), |el| {
                el.child(
                    div()
                        .h(px(160.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(muted("Queue is empty")),
                )
            })
            .child(
                list(
                    self.queue_list.clone(),
                    cx.processor(move |app, item: usize, _, cx| match &items[item] {
                        QueueItem::Header(section, count) => {
                            let section = *section;
                            let count = *count;
                            let open = match section {
                                "History" => app.preferences.history_open,
                                "Up next" => app.preferences.up_next_open,
                                _ => true,
                            };
                            div()
                                .id(SharedString::from(format!("queue-section-{section}")))
                                .test_support()
                                .role(if section == "Now playing" {
                                    Role::Label
                                } else {
                                    Role::Button
                                })
                                .aria_label(if section == "Now playing" {
                                    "Now playing".to_owned()
                                } else {
                                    format!("{section} {count}")
                                })
                                .h(px(if compact { 48. } else { 28. }))
                                .w_full()
                                .px_2()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_size(px(10.))
                                .line_height(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(MUTED))
                                .when(section != "Now playing", |el| {
                                    el.cursor_pointer().hover(|el| el.bg(rgb(SECONDARY))).child(
                                        glyph(
                                            if open {
                                                IconName::ChevronDown
                                            } else {
                                                IconName::ChevronRight
                                            },
                                            12.,
                                        ),
                                    )
                                })
                                .child(div().flex_1().child(section.to_uppercase()))
                                .when(section != "Now playing", |el| {
                                    el.child(
                                        div().font_family(mono_font()).child(count.to_string()),
                                    )
                                })
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    match section {
                                        "History" => {
                                            app.preferences.history_open =
                                                !app.preferences.history_open
                                        }
                                        "Up next" => {
                                            app.preferences.up_next_open =
                                                !app.preferences.up_next_open
                                        }
                                        _ => {}
                                    }
                                    app.save_preferences();
                                    cx.notify();
                                }))
                                .into_any_element()
                        }
                        QueueItem::Track(index) => {
                            let index = *index;
                            let track = queue.tracks[index].clone();
                            app.thumbnail(track.id, cx);
                            let label = title(&track);
                            let drag_label = label.clone();
                            let drag_view = cx.entity();
                            let row = div()
                                .id(("queue-track", index))
                                .relative()
                                .top(px(drag_shift(
                                    app.sort_drag,
                                    SortList::Queue,
                                    index,
                                    if compact { 64. } else { 56. },
                                )))
                                .when(
                                    app.sort_drag.is_some_and(|d| {
                                        d.list == SortList::Queue && d.from == index
                                    }),
                                    |el| el.opacity(0.),
                                )
                                .test_support()
                                .group("queue-track")
                                .h(px(if compact { 64. } else { 56. }))
                                .w_full()
                                .p_1()
                                .rounded(px(12.))
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(index as i64 == queue.cursor, |el| el.bg(rgb(SELECTED)))
                                .child(
                                    div()
                                        .size(px(48.))
                                        .flex_shrink_0()
                                        .rounded(px(10.))
                                        .overflow_hidden()
                                        .bg(rgb(BG))
                                        .when(
                                            app.library.read(cx).cached.contains(&track.id),
                                            |el| el.border_1().border_color(rgb(PRIMARY)),
                                        )
                                        .when_some(
                                            app.thumbs.get(&track.id).cloned().flatten(),
                                            |el, path| {
                                                el.child(
                                                    img(path)
                                                        .image_cache(&app.artwork_cache)
                                                        .size_full()
                                                        .object_fit(ObjectFit::Cover)
                                                        .rounded(px(8.)),
                                                )
                                            },
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .id(("queue-title", index))
                                                .test_support()
                                                .hover_title(label.clone(), &app.hover_title_epoch)
                                                .truncate()
                                                .when(index as i64 == queue.cursor, |el| {
                                                    el.text_color(rgb(PRIMARY))
                                                })
                                                .child(label),
                                        )
                                        .child(
                                            muted(artist(&track)).text_xs().truncate().hover_title(
                                                artist(&track),
                                                &app.hover_title_epoch,
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .opacity(0.)
                                        .group_hover("queue-track", |el| el.opacity(1.))
                                        .child(
                                            icon(("queue-remove", index), IconName::X).on_click(
                                                cx.listener(move |app, _, _, cx| {
                                                    cx.stop_propagation();
                                                    app.command(
                                                        Command::Remove {
                                                            indices: vec![index as i64],
                                                            queue_revision: revision,
                                                        },
                                                        cx,
                                                    );
                                                }),
                                            ),
                                        ),
                                )
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.command(
                                        Command::Jump {
                                            index: index as i64,
                                            queue_revision: revision,
                                        },
                                        cx,
                                    )
                                }))
                                .on_drag(QueueDrag { index, revision }, {
                                    let performer = artist(&track);
                                    let cover = app.thumbs.get(&track.id).cloned().flatten();
                                    move |_, _, window, cx| {
                                        let artwork_cache = drag_view.update(cx, |app, cx| {
                                            app.sort_drag = Some(SortDrag {
                                                list: SortList::Queue,
                                                from: index,
                                                over: index,
                                            });
                                            cx.notify();
                                            app.artwork_cache.clone()
                                        });
                                        let start_x = window.mouse_position().x;
                                        cx.new(|_| DragPreview {
                                            artwork_cache,
                                            label: drag_label.clone(),
                                            artist: Some(performer.clone()),
                                            cover: cover.clone(),
                                            width: 312.,
                                            height: if compact { 64. } else { 56. },
                                            start_x,
                                            kind: SortList::Queue,
                                            compact,
                                            trailing: String::new(),
                                        })
                                    }
                                })
                                .into_any_element();
                            div()
                                .h(px(if compact { 64. } else { 56. }))
                                .w_full()
                                .on_drag_move(cx.listener(
                                    move |app, event: &DragMoveEvent<QueueDrag>, _, cx| {
                                        if !event.bounds.contains(&event.event.position) {
                                            return;
                                        }
                                        let drag = event.drag(cx);
                                        if drag.revision == revision
                                            && app.sort_drag.is_none_or(|d| d.over != index)
                                        {
                                            app.sort_drag = Some(SortDrag {
                                                list: SortList::Queue,
                                                from: drag.index,
                                                over: index,
                                            });
                                            cx.notify();
                                        }
                                    },
                                ))
                                .on_drop(cx.listener(move |app, drag: &QueueDrag, _, cx| {
                                    app.sort_drag = None;
                                    if drag.revision == revision {
                                        app.command(
                                            Command::Reorder {
                                                from_index: drag.index as i64,
                                                to_index: index as i64,
                                                queue_revision: revision,
                                            },
                                            cx,
                                        );
                                    }
                                }))
                                .child(row)
                                .into_any_element()
                        }
                    }),
                )
                .h(px(if self.queue_items.is_empty() {
                    0.
                } else {
                    392.
                }))
                .px_1()
                .py_2()
                .w_full(),
            )
            .into_any_element()
    }
}
