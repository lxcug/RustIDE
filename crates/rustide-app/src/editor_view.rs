use std::ops::Range;

use eframe::egui;
use egui::epaint::text::cursor::CCursor;
use egui::{Align2, Color32, FontId, Rect, Stroke, TextStyle};
use rustide_syntax::SyntaxState;
use tree_sitter::{InputEdit, Point};

#[derive(Debug, Default, Clone, Copy)]
struct DragState {
    anchor: Option<usize>,
}

#[derive(Debug, Default, Clone, Copy)]
struct ClickState {
    last_time: f64,
    last_pos: usize,
    count: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EditorScrollMetrics {
    pub offset_y: f32,
    pub content_h: f32,
    pub viewport_h: f32,
    pub cursor_moved_by_pointer: bool,
}

pub fn show_editor(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    editor: &mut rustide_editor::Editor,
    max_line_chars: usize,
    syntax: &mut Option<SyntaxState>,
    theme: &crate::theme::Theme,
    ui_cfg: &mut crate::config::UiConfig,
    scroll_to_char: &mut Option<usize>,
) -> EditorScrollMetrics {
    let available = ui.available_size();
    let mut out_metrics = EditorScrollMetrics::default();

    // Render only the visible rows (plus a small margin) to keep scrolling responsive on large files.
    ui.push_id("rustide_editor_view", |ui| {
        let drag_id = ui.make_persistent_id("drag_state");
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click());
        if response.clicked() {
            response.request_focus();
        }

        if response.has_focus() {
            handle_input(ctx, editor, syntax);
            ctx.request_repaint();
        }

        let resizable_minimap_width = ui_cfg.minimap_width.clamp(40.0, 220.0);
        let minimap_width = resizable_minimap_width.min(rect.width() * 0.4).max(0.0);
        let main_rect = Rect::from_min_max(
            rect.min,
            egui::pos2((rect.max.x - minimap_width).max(rect.min.x), rect.max.y),
        );
        let minimap_rect = Rect::from_min_max(egui::pos2(main_rect.max.x, rect.min.y), rect.max);
        let minimap_enabled = minimap_rect.width() > 4.0 && minimap_rect.height() > 4.0;

        let minimap_id = ui.make_persistent_id("minimap");
        let minimap_response = ui.interact(minimap_rect, minimap_id, egui::Sense::click_and_drag());
        if minimap_response.clicked() || minimap_response.dragged() {
            response.request_focus();
        }

        if minimap_enabled {
            let handle_rect = Rect::from_min_max(
                egui::pos2(minimap_rect.left() - 3.0, minimap_rect.top()),
                egui::pos2(minimap_rect.left() + 3.0, minimap_rect.bottom()),
            );
            let handle_id = ui.make_persistent_id("minimap_resize");
            let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());
            if handle_resp.dragged() {
                let delta = handle_resp.drag_delta().x;
                ui_cfg.minimap_width = (ui_cfg.minimap_width - delta).clamp(40.0, 220.0);
                ctx.request_repaint();
            }
            if handle_resp.hovered() || handle_resp.dragged() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
            }
        }

        let font_id = TextStyle::Monospace.resolve(ui.style());
        // Ensure mixed CJK/Latin rows have consistent top/bottom spacing by using a row height
        // large enough for typical CJK glyphs (which may come from a fallback font).
        let base_row_height = ui.text_style_height(&TextStyle::Monospace);
        let cjk_row_height = ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap("国".to_owned(), font_id.clone(), Color32::PLACEHOLDER)
                .rect
                .height()
        });
        let row_height = base_row_height.max(cjk_row_height);
        let total_rows = editor.rope().len_lines();

        let desired_scroll_y = if let Some(char_idx) = *scroll_to_char {
            let len = editor.rope().len_chars();
            let clamped = char_idx.min(len);
            let line = editor.rope().char_to_line(clamped);
            let center = main_rect.height() * 0.5;
            Some((line as f32 * row_height - center).max(0.0))
        } else if minimap_enabled
            && (minimap_response.dragged()
                || (minimap_response.clicked() && minimap_response.hovered()))
        {
            minimap_response.interact_pointer_pos().map(|pos| {
                let height = minimap_rect.height().max(1.0);
                let t = ((pos.y - minimap_rect.top()) / height).clamp(0.0, 1.0);
                let target_row = (t * total_rows.saturating_sub(1) as f32).round().max(0.0) as f32;
                let center = main_rect.height() * 0.5;
                (target_row * row_height - center).max(0.0)
            })
        } else {
            None
        };

        let mut metrics = EditorScrollMetrics::default();
        let mut pointer_moved_cursor = false;

        ui.scope_builder(egui::UiBuilder::new().max_rect(main_rect), |ui| {
            let char_width = ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap("W".to_owned(), font_id.clone(), Color32::PLACEHOLDER)
                    .rect
                    .width()
            });
            let line_digits = total_rows.max(1).to_string().len();
            let gutter_width = line_digits as f32 * char_width + 12.0;
            let desired_width =
                (gutter_width + max_line_chars as f32 * char_width + 8.0).max(ui.available_width());

            if let Some(syntax) = syntax.as_mut() {
                let _ = syntax.ensure_parsed(editor.rope());
            }

            let mut scroll_area = egui::ScrollArea::both()
                .auto_shrink([false, false])
                .id_salt("rustide_editor_scroll");
            if let Some(y) = desired_scroll_y {
                scroll_area = scroll_area.vertical_scroll_offset(y);
            }

            let scroll_output =
                scroll_area.show_rows(ui, row_height, total_rows, |ui, row_range| {
                    ui.set_min_width(desired_width);

                    let (pointer_pos, pointer_down) =
                        ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down()));
                    let drag_anchor =
                        ctx.data(|d| d.get_temp::<DragState>(drag_id).and_then(|s| s.anchor));
                    let mut drag_target: Option<(f32, usize)> = None;

                    let selection = editor.selection().range();
                    let cursor = editor.selection().cursor;
                    let has_focus = response.has_focus();

                    for line_index in row_range {
                        let line_start = editor.rope().line_to_char(line_index);
                        let line_start_byte = editor.rope().char_to_byte(line_start);
                        let (line_text, line_len_chars) =
                            rope_line_without_newline(editor.rope(), line_index);
                        let line_end_char =
                            (line_start + line_len_chars).min(editor.rope().len_chars());
                        let line_end_byte = editor.rope().char_to_byte(line_end_char);
                        let highlight_spans = syntax
                            .as_mut()
                            .and_then(|s| {
                                s.highlight_spans(editor.rope(), line_start_byte..line_end_byte)
                                    .ok()
                            })
                            .unwrap_or_default();

                        let (row_rect, row_response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), row_height),
                            egui::Sense::click_and_drag(),
                        );

                        let local_selection =
                            selection_on_line(&selection, line_start, line_len_chars);

                        let mut job = egui::text::LayoutJob::default();
                        job.wrap.max_width = f32::INFINITY;

                        append_styled_line(
                            &mut job,
                            &line_text,
                            StyledLineArgs {
                                selection: local_selection,
                                font_id: &font_id,
                                selection_bg: ui.visuals().selection.bg_fill,
                                highlight_spans: &highlight_spans,
                                line_start_byte,
                                syntax_colors: &theme.syntax,
                            },
                        );

                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                        let y_offset = ((row_height - galley.rect.height()).max(0.0) * 0.5).round();
                        let text_origin = row_rect.min + egui::vec2(gutter_width, y_offset);

                        let line_number =
                            format!("{:>width$}", line_index + 1, width = line_digits);
                        ui.painter().text(
                            egui::pos2(
                                row_rect.min.x + gutter_width - 4.0,
                                row_rect.min.y + y_offset,
                            ),
                            Align2::RIGHT_TOP,
                            line_number,
                            font_id.clone(),
                            ui.visuals().weak_text_color(),
                        );
                        ui.painter()
                            .galley(text_origin, galley.clone(), ui.visuals().text_color());

                        if has_focus {
                            if let Some(local_cursor) =
                                cursor_on_line(cursor, line_start, line_len_chars)
                            {
                                let caret_rect = galley.pos_from_cursor(CCursor::new(local_cursor));
                                paint_caret(ui, text_origin, caret_rect);
                            }
                        }

                        if row_response.clicked() {
                            if let Some(pointer_pos) = row_response.interact_pointer_pos() {
                                let local = galley.cursor_from_pos(pointer_pos - text_origin).index;
                                let extend = ui.input(|i| i.modifiers.shift);
                                editor.set_cursor(
                                    (line_start + local).min(line_start + line_len_chars),
                                    extend,
                                );
                                pointer_moved_cursor = true;
                            }
                        }

                        if row_response.drag_started() {
                            if let Some(pointer_pos) = row_response.interact_pointer_pos() {
                                let local = galley.cursor_from_pos(pointer_pos - text_origin).index;
                                let anchor = (line_start + local).min(line_start + line_len_chars);
                                editor.set_cursor(anchor, false);
                                pointer_moved_cursor = true;
                                ctx.data_mut(|d| {
                                    d.insert_temp(
                                        drag_id,
                                        DragState {
                                            anchor: Some(anchor),
                                        },
                                    );
                                });
                            }
                        }

                        if let (Some(pointer_pos), true, Some(_anchor)) =
                            (pointer_pos, pointer_down, drag_anchor)
                        {
                            let y_dist = if pointer_pos.y < row_rect.top() {
                                row_rect.top() - pointer_pos.y
                            } else if pointer_pos.y > row_rect.bottom() {
                                pointer_pos.y - row_rect.bottom()
                            } else {
                                0.0
                            };
                            let local = galley.cursor_from_pos(pointer_pos - text_origin).index;
                            let target = (line_start + local).min(line_start + line_len_chars);
                            match drag_target {
                                None => drag_target = Some((y_dist, target)),
                                Some((best_dist, _)) if y_dist < best_dist => {
                                    drag_target = Some((y_dist, target));
                                }
                                _ => {}
                            }
                        }
                    }

                    if let (Some(_anchor), Some((_dist, target)), true) =
                        (drag_anchor, drag_target, pointer_down)
                    {
                        editor.set_cursor(target, true);
                        pointer_moved_cursor = true;
                    }
                    if !pointer_down {
                        ctx.data_mut(|d| d.insert_temp(drag_id, DragState::default()));
                    }
                });

            metrics.offset_y = scroll_output.state.offset.y;
            metrics.content_h = scroll_output.content_size.y;
            metrics.viewport_h = scroll_output.inner_rect.height();

            let (pointer_pos, pointer_clicked, extend) = ui.input(|i| {
                (
                    i.pointer.interact_pos(),
                    i.pointer.primary_clicked(),
                    i.modifiers.shift,
                )
            });

            if let Some(pointer_pos) = pointer_pos {
                if scroll_output.inner_rect.contains(pointer_pos) {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
                }
            }

            if let (Some(pointer_pos), true) = (pointer_pos, pointer_clicked) {
                if scroll_output.inner_rect.contains(pointer_pos) {
                    response.request_focus();

                    let row_height_with_spacing = row_height + ui.spacing().item_spacing.y;
                    let content_pos =
                        pointer_pos - scroll_output.inner_rect.min + scroll_output.state.offset;
                    let line_index =
                        (content_pos.y / row_height_with_spacing).floor().max(0.0) as usize;
                    let line_index = line_index.min(total_rows.saturating_sub(1));

                    let line_start = editor.rope().line_to_char(line_index);
                    let (line_text, line_len_chars) =
                        rope_line_without_newline(editor.rope(), line_index);

                    let local_x = (content_pos.x - gutter_width).max(0.0);
                    let galley = ui.fonts_mut(|fonts| {
                        fonts.layout_no_wrap(
                            line_text.clone(),
                            font_id.clone(),
                            Color32::PLACEHOLDER,
                        )
                    });
                    let y_offset = ((row_height - galley.rect.height()).max(0.0) * 0.5).round();
                    let local_y = (content_pos.y - line_index as f32 * row_height_with_spacing)
                        .clamp(0.0, row_height)
                        - y_offset;
                    let local_y = local_y.max(0.0);
                    let local = galley.cursor_from_pos(egui::vec2(local_x, local_y)).index;
                    let clicked_pos = (line_start + local).min(line_start + line_len_chars);

                    let click_id = ui.make_persistent_id("click_state");
                    let now = ui.input(|i| i.time);
                    let mut state =
                        ctx.data(|d| d.get_temp::<ClickState>(click_id).unwrap_or_default());
                    if now - state.last_time <= 0.45 && clicked_pos.abs_diff(state.last_pos) <= 2 {
                        state.count = state.count.saturating_add(1).min(3);
                    } else {
                        state.count = 1;
                    }
                    state.last_time = now;
                    state.last_pos = clicked_pos;
                    ctx.data_mut(|d| d.insert_temp(click_id, state));

                    match state.count {
                        2 => select_word(editor, clicked_pos),
                        3 => select_line(editor, line_index),
                        _ => editor.set_cursor(clicked_pos, extend),
                    }
                    pointer_moved_cursor = true;
                }
            }

            metrics.cursor_moved_by_pointer = pointer_moved_cursor;
        });

        if minimap_enabled {
            paint_minimap(
                ui,
                editor,
                total_rows,
                minimap_rect,
                minimap_response,
                metrics,
                &theme.minimap,
            );
        }

        out_metrics = metrics;
    });

    if scroll_to_char.is_some() {
        // Consume the request so we don't keep forcing the scroll offset every frame.
        *scroll_to_char = None;
    }

    out_metrics
}

fn handle_input(
    ctx: &egui::Context,
    editor: &mut rustide_editor::Editor,
    syntax: &mut Option<SyntaxState>,
) {
    let events = ctx.input(|i| i.events.clone());
    for event in events {
        match event {
            egui::Event::Copy => {
                let text = editor.selected_text();
                if !text.is_empty() {
                    ctx.copy_text(text);
                }
            }
            egui::Event::Cut => {
                let text = editor.selected_text();
                if !text.is_empty() {
                    ctx.copy_text(text);
                    editor.insert_text("");
                    drain_syntax_edits(editor, syntax);
                }
            }
            egui::Event::Paste(text) => {
                editor.insert_text(&text);
                drain_syntax_edits(editor, syntax);
            }
            egui::Event::Text(text) => {
                editor.insert_text(&text);
                drain_syntax_edits(editor, syntax);
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                if handle_key(editor, key, modifiers) {
                    drain_syntax_edits(editor, syntax);
                    // keep going: multiple keys can be pressed in one frame
                }
            }
            _ => {}
        }
    }
}

fn handle_key(
    editor: &mut rustide_editor::Editor,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> bool {
    let command = modifiers.command;
    let extend = modifiers.shift;

    if command {
        match key {
            egui::Key::Z if extend => return editor.redo(),
            egui::Key::Z => return editor.undo(),
            egui::Key::Y => return editor.redo(),
            egui::Key::A => {
                editor.select_all();
                return true;
            }
            _ => {}
        }
    }

    match key {
        egui::Key::ArrowLeft => {
            editor.move_left(extend);
            true
        }
        egui::Key::ArrowRight => {
            editor.move_right(extend);
            true
        }
        egui::Key::ArrowUp => {
            editor.move_up(extend);
            true
        }
        egui::Key::ArrowDown => {
            editor.move_down(extend);
            true
        }
        egui::Key::Home => {
            editor.move_line_start(extend);
            true
        }
        egui::Key::End => {
            editor.move_line_end(extend);
            true
        }
        egui::Key::Backspace => {
            editor.backspace();
            true
        }
        egui::Key::Delete => {
            editor.delete_forward();
            true
        }
        egui::Key::Enter => {
            editor.insert_newline_auto_indent();
            true
        }
        egui::Key::Tab => {
            editor.insert_text("    ");
            true
        }
        _ => false,
    }
}

fn rope_line_without_newline(rope: &ropey::Rope, line_index: usize) -> (String, usize) {
    let slice = rope.line(line_index);
    let mut text = slice.to_string();
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    let len_chars = text.chars().count();
    (text, len_chars)
}

fn cursor_on_line(cursor: usize, line_start: usize, line_len_chars: usize) -> Option<usize> {
    if cursor < line_start {
        return None;
    }
    let local = cursor - line_start;
    if local <= line_len_chars {
        Some(local)
    } else {
        None
    }
}

fn selection_on_line(
    selection: &Range<usize>,
    line_start: usize,
    line_len_chars: usize,
) -> Option<Range<usize>> {
    if selection.is_empty() {
        return None;
    }
    let line_end = line_start + line_len_chars;
    let start = selection.start.max(line_start).min(line_end);
    let end = selection.end.max(line_start).min(line_end);
    if start < end {
        Some((start - line_start)..(end - line_start))
    } else {
        None
    }
}

struct StyledLineArgs<'a> {
    selection: Option<Range<usize>>,
    font_id: &'a FontId,
    selection_bg: Color32,
    highlight_spans: &'a [rustide_syntax::HighlightSpan],
    line_start_byte: usize,
    syntax_colors: &'a crate::theme::SyntaxColors,
}

fn append_styled_line(job: &mut egui::text::LayoutJob, line: &str, args: StyledLineArgs<'_>) {
    let StyledLineArgs {
        selection,
        font_id,
        selection_bg,
        highlight_spans,
        line_start_byte,
        syntax_colors,
    } = args;
    let selection_bytes = selection.map(|sel| {
        let start = char_to_byte_index(line, sel.start);
        let end = char_to_byte_index(line, sel.end);
        start..end
    });

    let mut boundaries: Vec<usize> = vec![0, line.len()];
    if let Some(sel) = &selection_bytes {
        boundaries.push(sel.start);
        boundaries.push(sel.end);
    }
    for span in highlight_spans {
        let rel_start = span.byte_range.start.saturating_sub(line_start_byte);
        let rel_end = span.byte_range.end.saturating_sub(line_start_byte);
        boundaries.push(rel_start.min(line.len()));
        boundaries.push(rel_end.min(line.len()));
    }

    boundaries.sort_unstable();
    boundaries.dedup();

    for w in boundaries.windows(2) {
        let start = w[0];
        let end = w[1].min(line.len());
        if start >= end {
            continue;
        }

        let fg = highlight_spans
            .iter()
            .find(|s| {
                let rs = s.byte_range.start.saturating_sub(line_start_byte);
                let re = s.byte_range.end.saturating_sub(line_start_byte);
                rs <= start && start < re
            })
            .map(|s| syntax_colors.for_tag(s.tag))
            .unwrap_or(syntax_colors.fallback);

        let selected = selection_bytes
            .as_ref()
            .is_some_and(|sel| sel.start <= start && end <= sel.end);

        let fmt = egui::TextFormat {
            font_id: font_id.clone(),
            color: fg,
            background: if selected {
                selection_bg
            } else {
                Color32::TRANSPARENT
            },
            ..Default::default()
        };
        job.append(&line[start..end], 0.0, fmt);
    }
}

fn drain_syntax_edits(editor: &mut rustide_editor::Editor, syntax: &mut Option<SyntaxState>) {
    let Some(syntax) = syntax.as_mut() else {
        let _ = editor.take_last_edit();
        return;
    };
    while let Some(edit) = editor.take_last_edit() {
        syntax.queue_edit(to_input_edit(edit));
    }
}

fn to_input_edit(edit: rustide_editor::EditorEdit) -> InputEdit {
    InputEdit {
        start_byte: edit.start_byte,
        old_end_byte: edit.old_end_byte,
        new_end_byte: edit.new_end_byte,
        start_position: Point {
            row: edit.start_point.row,
            column: edit.start_point.column,
        },
        old_end_position: Point {
            row: edit.old_end_point.row,
            column: edit.old_end_point.column,
        },
        new_end_position: Point {
            row: edit.new_end_point.row,
            column: edit.new_end_point.column,
        },
    }
}

fn paint_minimap(
    ui: &egui::Ui,
    editor: &rustide_editor::Editor,
    total_rows: usize,
    rect: Rect,
    response: egui::Response,
    metrics: EditorScrollMetrics,
    colors: &crate::theme::MinimapColors,
) {
    let EditorScrollMetrics {
        offset_y,
        content_h,
        viewport_h,
        ..
    } = metrics;

    if rect.is_negative() || rect.area() <= 1.0 {
        return;
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, colors.background);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, colors.border),
        egui::StrokeKind::Inside,
    );

    let font_size = 6.0_f32.min(rect.height().max(4.0)).max(4.0);
    let step_y = font_size;
    let approx_char_w = font_size * 0.6;
    let max_chars = if approx_char_w <= f32::EPSILON {
        16
    } else {
        (((rect.width() - 4.0).max(0.0)) / approx_char_w)
            .floor()
            .clamp(8.0, 120.0) as usize
    };

    let rope = editor.rope();
    let sample_count = (rect.height() / step_y).ceil().max(1.0) as usize;
    for i in 0..sample_count {
        let y = rect.top() + i as f32 * step_y;
        if y > rect.bottom() {
            break;
        }
        let t = ((y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        let line_index = if total_rows <= 1 {
            0
        } else {
            (t * (total_rows.saturating_sub(1) as f32)).round() as usize
        };
        if line_index >= total_rows {
            continue;
        }
        let snippet = rope_line_snippet(rope, line_index, max_chars);
        if snippet.is_empty() {
            continue;
        }
        painter.text(
            egui::pos2(rect.left() + 2.0, y),
            Align2::LEFT_TOP,
            snippet,
            FontId::monospace(font_size),
            colors.text,
        );
    }

    let content_h = content_h.max(1.0);
    let viewport_h = viewport_h.max(1.0);
    let max_thumb = rect.height().max(0.0);
    let min_thumb = 16.0_f32.min(max_thumb);
    let thumb_h = (viewport_h / content_h * rect.height()).clamp(min_thumb, max_thumb);
    let max_scroll = (content_h - viewport_h).max(0.0);
    let scroll_y = offset_y.clamp(0.0, max_scroll.max(0.0001));
    let thumb_y = if max_scroll <= f32::EPSILON {
        0.0
    } else {
        (scroll_y / max_scroll) * (rect.height() - thumb_h)
    };
    let thumb = Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + thumb_y),
        egui::vec2(rect.width(), thumb_h),
    );
    painter.rect_filled(thumb, 2.0, colors.viewport_fill);
    painter.rect_stroke(
        thumb,
        2.0,
        Stroke::new(1.0, colors.viewport_stroke),
        egui::StrokeKind::Inside,
    );

    let cursor_line = editor.rope().char_to_line(editor.selection().cursor);
    let t = if total_rows <= 1 {
        0.0
    } else {
        cursor_line as f32 / (total_rows.saturating_sub(1) as f32)
    };
    let y = rect.top() + t * rect.height();
    painter.line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        Stroke::new(1.0, colors.caret_marker),
    );
}

fn rope_line_snippet(rope: &ropey::Rope, line_index: usize, max_chars: usize) -> String {
    let slice = rope.line(line_index);
    let mut out = String::new();
    let mut count = 0usize;
    for ch in slice.chars() {
        if ch == '\n' || ch == '\r' {
            break;
        }
        out.push(ch);
        count += 1;
        if count >= max_chars {
            break;
        }
    }
    out
}

fn select_word(editor: &mut rustide_editor::Editor, pos: usize) {
    let rope = editor.rope();
    if rope.len_chars() == 0 {
        return;
    }
    let pos = pos.min(rope.len_chars().saturating_sub(1));
    let ch = rope.char(pos);
    if ch.is_whitespace() {
        editor.set_cursor(pos, false);
        return;
    }

    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let target_is_word = is_word(ch);

    let mut start = pos;
    while start > 0 {
        let c = rope.char(start - 1);
        if c.is_whitespace() || is_word(c) != target_is_word {
            break;
        }
        start -= 1;
    }

    let mut end = pos + 1;
    while end < rope.len_chars() {
        let c = rope.char(end);
        if c.is_whitespace() || is_word(c) != target_is_word {
            break;
        }
        end += 1;
    }

    editor.select_range(start..end);
}

fn select_line(editor: &mut rustide_editor::Editor, line_index: usize) {
    let rope = editor.rope();
    let line_index = line_index.min(rope.len_lines().saturating_sub(1));
    let start = rope.line_to_char(line_index);
    let slice = rope.line(line_index);
    let mut len = slice.len_chars();
    if len > 0 && slice.char(len - 1) == '\n' {
        len -= 1;
        if len > 0 && slice.char(len - 1) == '\r' {
            len -= 1;
        }
    }
    editor.select_range(start..(start + len));
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    match text.char_indices().nth(char_index) {
        Some((byte, _)) => byte,
        None => text.len(),
    }
}

fn paint_caret(ui: &egui::Ui, text_origin: egui::Pos2, caret_rect: Rect) {
    let caret_color = ui.visuals().text_color();
    let stroke = Stroke::new(2.0, caret_color);
    let x = text_origin.x + caret_rect.min.x;
    let top = text_origin.y + caret_rect.min.y;
    let bottom = text_origin.y + caret_rect.max.y;
    ui.painter()
        .line_segment([egui::pos2(x, top), egui::pos2(x, bottom)], stroke);
}
