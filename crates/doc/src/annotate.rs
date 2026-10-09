//! Node labels (a `// comment` line right above a node) and disabling nodes with KDL's `/-`.

use std::ops::Range;

use anyhow::{Context as _, Result};

use crate::{Document, NodeRef};

/// A top-level node commented out with `/-`. kdl-rs doesn't expose these, so they are found
/// by scanning the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Disabled {
    pub file: usize,
    /// From the `/-` to the end of the node.
    pub range: Range<usize>,
    /// The node's own text, without the `/-`.
    pub body: String,
}

impl Document {
    /// The label of the node at `offset`: the text of a `//` comment on the line just above.
    pub fn label_at(&self, file: usize, offset: usize) -> Option<String> {
        let text = &self.files[file].text;
        let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
        if line_start == 0 {
            return None;
        }
        let prev_start = text[..line_start - 1].rfind('\n').map_or(0, |i| i + 1);
        let prev = text[prev_start..line_start - 1].trim();
        let label = prev.strip_prefix("//")?.trim();
        // A label is a lone comment line; the tail of a comment paragraph is prose, not a name.
        if prev_start > 0 {
            let before_start = text[..prev_start - 1].rfind('\n').map_or(0, |i| i + 1);
            if text[before_start..prev_start - 1].trim().starts_with("//") {
                return None;
            }
        }
        (!label.is_empty() && !prev.starts_with("///") && label.len() <= 80).then(|| label.to_owned())
    }

    pub fn label(&self, r: &NodeRef) -> Option<String> {
        let offset = self.node(r)?.span().offset();
        self.label_at(r.file, offset)
    }

    /// Sets, replaces or (with `None`) removes the label line above the node at `offset`.
    pub fn set_label_at(&mut self, file: usize, offset: usize, label: Option<&str>) {
        let label = label.map(|l| l.replace('\n', " ")).filter(|l| !l.trim().is_empty());
        let text = &self.files[file].text;
        let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
        let indent: String = text[line_start..offset].chars().take_while(|c| c.is_whitespace()).collect();
        let existing = self.label_at(file, offset).is_some();
        let prev_start = if line_start == 0 { 0 } else { text[..line_start - 1].rfind('\n').map_or(0, |i| i + 1) };
        let (range, with) = match (existing, label) {
            (true, Some(l)) => (prev_start..line_start, format!("{indent}// {}\n", l.trim())),
            (true, None) => (prev_start..line_start, String::new()),
            (false, Some(l)) => (line_start..line_start, format!("{indent}// {}\n", l.trim())),
            (false, None) => return,
        };
        self.checkpoint(&format!("label:{file}:{offset}"));
        self.splice(file, range, &with);
    }

    pub fn set_label(&mut self, r: &NodeRef, label: Option<&str>) -> Result<()> {
        let offset = self.node(r).context("node not found")?.span().offset();
        self.set_label_at(r.file, offset, label);
        Ok(())
    }

    /// The node's text range including its label line, if any.
    pub fn block_range(&self, r: &NodeRef) -> Option<Range<usize>> {
        let span = self.node(r)?.span();
        let text = &self.files[r.file].text;
        let mut start = span.offset();
        if self.label_at(r.file, start).is_some() {
            let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
            start = text[..line_start - 1].rfind('\n').map_or(0, |i| i + 1);
            // Keep the first node's indentation where it was.
            start += text[start..].len() - text[start..].trim_start_matches([' ', '\t']).len();
        }
        Some(start..span.offset() + span.len())
    }

    /// Swaps two sibling nodes together with their labels.
    pub fn swap_blocks(&mut self, a: &NodeRef, b: &NodeRef) -> Result<()> {
        anyhow::ensure!(a.file == b.file, "nodes are in different files");
        let ra = self.block_range(a).context("node not found")?;
        let rb = self.block_range(b).context("node not found")?;
        let (r1, r2) = if ra.start < rb.start { (ra, rb) } else { (rb, ra) };
        let t1 = self.files[a.file].text[r1.clone()].to_owned();
        let t2 = self.files[a.file].text[r2.clone()].to_owned();
        self.checkpoint("swap");
        self.splice(a.file, r2, &t1);
        self.splice(a.file, r1, &t2);
        Ok(())
    }

    /// Comments the node out with `/-`, keeping it in the file.
    pub fn disable(&mut self, r: &NodeRef) -> Result<()> {
        let offset = self.node(r).context("node not found")?.span().offset();
        self.checkpoint("disable");
        self.splice(r.file, offset..offset, "/-");
        Ok(())
    }

    pub fn enable(&mut self, d: &Disabled) {
        let text = &self.files[d.file].text;
        let after = d.range.start + 2;
        let ws = text[after..].len() - text[after..].trim_start().len();
        self.checkpoint("enable");
        self.splice(d.file, d.range.start..after + ws, "");
    }

    /// Deletes a disabled node, including its whole line(s) and its label.
    pub fn remove_disabled(&mut self, d: &Disabled) {
        let text = &self.files[d.file].text;
        let mut start = text[..d.range.start].rfind('\n').map_or(0, |i| i + 1);
        let own_line = text[start..d.range.start].trim().is_empty();
        if !own_line {
            start = d.range.start;
        }
        let mut end = d.range.end;
        if let Some(nl) = text[end..].find('\n') {
            if text[end..end + nl].trim().is_empty() {
                end += nl + 1;
            }
        }
        if own_line && start > 0 && self.label_at(d.file, d.range.start).is_some() {
            start = text[..start - 1].rfind('\n').map_or(0, |i| i + 1);
        }
        self.checkpoint("remove-disabled");
        self.splice(d.file, start..end, "");
    }

    /// Top-level nodes named `name` that are commented out with `/-`.
    pub fn disabled_nodes(&self, name: &str) -> Vec<Disabled> {
        let mut out = Vec::new();
        for (fi, f) in self.files.iter().enumerate() {
            let text = &f.text;
            let mut depth = 0i32;
            let mut i = 0;
            let bytes = text.as_bytes();
            while i < bytes.len() {
                match skip_trivia(text, i) {
                    Some(next) => {
                        i = next;
                        continue;
                    }
                    None => {}
                }
                let c = bytes[i];
                if c == b'{' {
                    depth += 1;
                } else if c == b'}' {
                    depth -= 1;
                } else if depth == 0 && text[i..].starts_with("/-") {
                    let rest = &text[i + 2..];
                    let body_start = i + 2 + (rest.len() - rest.trim_start().len());
                    let after_name = &text[body_start..];
                    if after_name.starts_with(name)
                        && after_name[name.len()..].starts_with(|c: char| c.is_whitespace() || c == '{' || c == ';')
                    {
                        let end = node_end(text, body_start);
                        out.push(Disabled { file: fi, range: i..end, body: text[body_start..end].to_owned() });
                        i = end;
                        continue;
                    }
                }
                i += 1;
            }
        }
        out
    }
}

/// If a comment or string starts at `i`, the offset just past it.
fn skip_trivia(text: &str, i: usize) -> Option<usize> {
    let rest = &text[i..];
    if rest.starts_with("//") {
        return Some(rest.find('\n').map_or(text.len(), |n| i + n));
    }
    if rest.starts_with("/*") {
        return Some(rest[2..].find("*/").map_or(text.len(), |n| i + 2 + n + 2));
    }
    if rest.starts_with('"') {
        let b = text.as_bytes();
        let mut j = i + 1;
        while j < b.len() {
            match b[j] {
                b'\\' => j += 2,
                b'"' => return Some(j + 1),
                _ => j += 1,
            }
        }
        return Some(text.len());
    }
    if rest.starts_with("r\"") || rest.starts_with("r#") {
        let hashes = rest[1..].bytes().take_while(|&b| b == b'#').count();
        if rest[1 + hashes..].starts_with('"') {
            let close = format!("\"{}", "#".repeat(hashes));
            let body = i + 1 + hashes + 1;
            return Some(text[body..].find(&close).map_or(text.len(), |n| body + n + close.len()));
        }
    }
    None
}

/// End of the node starting at `start`: after its children block, or at its line's end.
fn node_end(text: &str, start: usize) -> usize {
    let b = text.as_bytes();
    let mut depth = 0i32;
    let mut i = start;
    while i < b.len() {
        if let Some(next) = skip_trivia(text, i) {
            i = next;
            continue;
        }
        match b[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            b'\n' | b';' if depth == 0 => return i,
            _ => {}
        }
        i += 1;
    }
    text.len()
}
