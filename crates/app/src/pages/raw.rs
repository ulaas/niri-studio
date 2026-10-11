//! The raw KDL editor: syntax highlighting, line numbers and niri's diagnostics inline.

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::{Color32, FontId, RichText, TextBuffer};

use crate::app::Ctx;

#[derive(Default)]
pub struct State {
    file: usize,
    buf: String,
    gen: Option<u64>,
    jump: Option<(usize, usize)>,
}

impl State {
    pub fn jump(&mut self, file: usize, line: usize) {
        self.file = file;
        self.gen = None;
        self.jump = Some((file, line));
    }
}

pub fn show(ui: &mut egui::Ui, cx: &mut Ctx, st: &mut State) {
    if st.file >= cx.doc.files.len() {
        st.file = 0;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Raw config").size(22.0).strong());
        ui.add_space(12.0);
        for (i, f) in cx.doc.files.iter().enumerate() {
            let name = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let name = if f.is_dirty() { format!("{name} •") } else { name };
            if ui.selectable_label(st.file == i, name).on_hover_text(f.path.display().to_string()).clicked() {
                st.file = i;
                st.gen = None;
            }
        }
    });
    if let Err(e) = &cx.doc.files[st.file].kdl {
        ui.colored_label(ui.visuals().warn_fg_color, format!("Syntax: {e}"));
    }
    ui.weak("Edits here and in the visual pages stay in sync, and share one undo history.");
    ui.add_space(4.0);

    if st.gen != Some(cx.doc.generation) {
        st.buf = cx.doc.files[st.file].text.clone();
        st.gen = Some(cx.doc.generation);
    }

    let errors: Vec<(Range<usize>, String)> = cx
        .doc
        .validation()
        .errors
        .iter()
        .filter(|e| e.file == Some(st.file))
        .filter_map(|e| Some((e.span.clone()?, e.message.clone())))
        .collect();
    let error_lines: Vec<usize> = cx
        .doc
        .validation()
        .errors
        .iter()
        .filter(|e| e.file == Some(st.file))
        .filter_map(|e| e.line)
        .collect();

    let font = FontId::monospace(13.5);
    let theme = crate::theme::pal(ui.ctx());
    let err_bg = ui.visuals().error_fg_color.gamma_multiply(0.25);
    let mut layouter = |ui: &egui::Ui, text: &dyn TextBuffer, _wrap: f32| -> Arc<egui::Galley> {
        let mut job = highlight(text.as_str(), &font, &theme, &errors, err_bg);
        job.wrap.max_width = f32::INFINITY;
        ui.fonts_mut(|f| f.layout_job(job))
    };

    let id = egui::Id::new(("raw-editor", st.file));
    let mut jump_to: Option<usize> = None;
    if let Some((file, line)) = st.jump.take() {
        if file == st.file {
            let char_idx = st.buf.lines().take(line.saturating_sub(1)).map(|l| l.chars().count() + 1).sum();
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), id) {
                state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(char_idx))));
                state.store(ui.ctx(), id);
            }
            ui.memory_mut(|m| m.request_focus(id));
            jump_to = Some(char_idx);
        }
    }

    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            let lines = st.buf.lines().count().max(1) + usize::from(st.buf.ends_with('\n'));
            let gutter: String = (1..=lines).map(|n| format!("{n:>4}\n")).collect();
            let mut job = LayoutJob::default();
            for (i, line) in gutter.lines().enumerate() {
                let color = if error_lines.contains(&(i + 1)) { ui.visuals().error_fg_color } else { ui.visuals().weak_text_color() };
                job.append(&format!("{line}\n"), 0.0, TextFormat::simple(font.clone(), color));
            }
            ui.add_space(2.0);
            ui.label(job);

            let out = egui::TextEdit::multiline(&mut st.buf)
                .id(id)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(40)
                .lock_focus(true)
                .layouter(&mut layouter)
                .show(ui);
            if out.response.changed() {
                cx.doc.set_text(st.file, st.buf.clone());
                st.gen = Some(cx.doc.generation);
            }
            if let Some(ci) = jump_to {
                let r = out.galley.pos_from_cursor(CCursor::new(ci)).translate(out.galley_pos.to_vec2());
                ui.scroll_to_rect(r.expand(60.0), Some(egui::Align::Center));
            }
        });
    });
}

struct Palette {
    text: Color32,
    comment: Color32,
    string: Color32,
    number: Color32,
    node: Color32,
    prop: Color32,
    keyword: Color32,
    punct: Color32,
}

/// Syntax colors from the app theme.
fn palette(t: &crate::theme::Palette) -> Palette {
    Palette {
        text: t.text,
        comment: t.faint,
        string: crate::theme::readable(t.ok, t.bg, 3.0),
        number: crate::theme::readable(t.warn, t.bg, 3.0),
        node: t.blue,
        prop: t.purple,
        keyword: t.error,
        punct: t.muted,
    }
}

/// A forgiving KDL v1 highlighter (it never fails, even on broken input).
fn highlight(text: &str, font: &FontId, theme: &crate::theme::Palette, errors: &[(Range<usize>, String)], err_bg: Color32) -> LayoutJob {
    let pal = palette(theme);
    let mut job = LayoutJob::default();
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut at_node_start = true;
    let push = |job: &mut LayoutJob, range: Range<usize>, color: Color32| {
        // Split at error boundaries so error spans get a background.
        let mut start = range.start;
        while start < range.end {
            let err = errors.iter().find(|(r, _)| r.start <= start && start < r.end.max(r.start + 1));
            let next_boundary = errors
                .iter()
                .flat_map(|(r, _)| [r.start, r.end.max(r.start + 1)])
                .filter(|&b| b > start && b < range.end)
                .min()
                .unwrap_or(range.end);
            let mut fmt = TextFormat::simple(font.clone(), color);
            if err.is_some() {
                fmt.background = err_bg;
                fmt.underline = egui::Stroke::new(1.5, Color32::from_rgb(230, 80, 80));
            }
            job.append(&text[start..next_boundary], 0.0, fmt);
            start = next_boundary;
        }
    };

    while i < bytes.len() {
        let c = bytes[i];
        let start = i;
        if text[i..].starts_with("//") {
            let end = text[i..].find('\n').map_or(text.len(), |n| i + n);
            push(&mut job, start..end, pal.comment);
            i = end;
        } else if text[i..].starts_with("/*") {
            let end = text[i + 2..].find("*/").map_or(text.len(), |n| i + 2 + n + 2);
            push(&mut job, start..end, pal.comment);
            i = end;
        } else if text[i..].starts_with("/-") {
            push(&mut job, start..i + 2, pal.comment);
            i += 2;
        } else if c == b'"' || (c == b'r' && matches!(bytes.get(i + 1), Some(b'"' | b'#'))) {
            let end = string_end(text, i);
            let color = if at_node_start { pal.node } else { pal.string };
            // A quoted property key is still a key.
            let color = if bytes.get(end) == Some(&b'=') { pal.prop } else { color };
            push(&mut job, start..end, color);
            at_node_start = false;
            i = end;
        } else if c == b'\n' || c == b';' || c == b'{' || c == b'}' {
            let color = if c == b'\n' { pal.text } else { pal.punct };
            push(&mut job, start..i + 1, color);
            at_node_start = true;
            i += 1;
        } else if c.is_ascii_whitespace() || c == b'=' || c == b'(' || c == b')' || c == b'\\' {
            push(&mut job, start..i + 1, pal.punct);
            i += 1;
        } else {
            let mut end = i;
            while end < bytes.len() && !b" \t\r\n;{}=()\"\\".contains(&bytes[end]) && !text[end..].starts_with("//") {
                end += text[end..].chars().next().map_or(1, char::len_utf8);
            }
            let word = &text[start..end];
            let color = if bytes.get(end) == Some(&b'=') {
                pal.prop
            } else if at_node_start {
                pal.node
            } else if matches!(word, "true" | "false" | "null") {
                pal.keyword
            } else if word.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+' || c == '.') {
                pal.number
            } else {
                pal.text
            };
            push(&mut job, start..end.max(start + 1), color);
            at_node_start = false;
            i = end.max(start + 1);
        }
    }
    job
}

fn string_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    if bytes[start] == b'r' {
        let hashes = text[start + 1..].bytes().take_while(|&b| b == b'#').count();
        let close = format!("\"{}", "#".repeat(hashes));
        let body = start + 1 + hashes + 1;
        return text.get(body..).and_then(|t| t.find(&close)).map_or(text.len(), |n| body + n + close.len());
    }
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    text.len()
}
