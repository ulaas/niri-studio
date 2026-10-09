//! Format-preserving editing of niri's KDL config.
//!
//! The text of each file is the source of truth. Every edit is a splice into that text, computed
//! from kdl-rs spans and rendered as KDL v1 by [`value`], followed by a re-parse. Untouched bytes,
//! comments and formatting are never rewritten. After each edit the text is re-validated with
//! niri's own parser ([`validate`]), so the app sees exactly what niri will see.

mod annotate;
mod files;
pub mod path;
pub mod validate;
pub mod value;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use kdl::{KdlDocument, KdlNode};

pub use annotate::Disabled;
pub use files::{format_time, list_backups, state_dir, Backup, SaveOutcome};
pub use path::{p, Seg};
pub use validate::{Diag, Validation};
pub use value::Val;

const INDENT: &str = "    ";
const UNDO_LIMIT: usize = 500;
/// Consecutive edits to the same setting within this window merge into one undo step,
/// so dragging a slider is undone in one go.
const COALESCE: Duration = Duration::from_millis(800);

pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
    /// kdl-rs parse; `Err` holds the message. Structured editing of this file is disabled
    /// until the text parses again (raw editing still works).
    pub kdl: Result<KdlDocument, String>,
    /// Text as last loaded from or saved to disk.
    saved: String,
}

impl SourceFile {
    fn new(path: PathBuf, text: String) -> Self {
        let kdl = parse(&text);
        Self { path, saved: text.clone(), text, kdl }
    }

    pub fn is_dirty(&self) -> bool {
        self.text != self.saved
    }

    pub fn saved_text(&self) -> &str {
        &self.saved
    }
}

fn parse(text: &str) -> Result<KdlDocument, String> {
    KdlDocument::parse_v1(text).map_err(|e| {
        e.diagnostics
            .first()
            .and_then(|d| d.message.clone())
            .unwrap_or_else(|| e.to_string())
    })
}

/// Location of a node: file index and the child index at each nesting level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeRef {
    pub file: usize,
    pub idx: Vec<usize>,
}

impl NodeRef {
    pub fn parent(&self) -> Option<NodeRef> {
        (self.idx.len() > 1).then(|| NodeRef {
            file: self.file,
            idx: self.idx[..self.idx.len() - 1].to_vec(),
        })
    }

    pub fn child(&self, i: usize) -> NodeRef {
        let mut idx = self.idx.clone();
        idx.push(i);
        NodeRef { file: self.file, idx }
    }
}

pub struct Document {
    /// `files[0]` is the main config; the rest are includes in load order.
    pub files: Vec<SourceFile>,
    undo: Vec<Vec<String>>,
    redo: Vec<Vec<String>>,
    last_edit: Option<(String, Instant)>,
    /// Bumped on every change, so views can cache derived data.
    pub generation: u64,
    validation: Option<Validation>,
}

impl Document {
    /// Loads the main config and everything it includes.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {path:?}"))?;
        let mut doc = Self::from_text(path.to_path_buf(), text);
        doc.discover_includes();
        Ok(doc)
    }

    pub fn from_text(path: PathBuf, text: String) -> Self {
        Self {
            files: vec![SourceFile::new(path, text)],
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            generation: 0,
            validation: None,
        }
    }

    /// The default config location: `$XDG_CONFIG_HOME/niri/config.kdl`.
    pub fn default_path() -> PathBuf {
        if let Some(p) = std::env::var_os("NIRI_CONFIG").filter(|p| !p.is_empty()) {
            return PathBuf::from(p);
        }
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home().join(".config"));
        base.join("niri/config.kdl")
    }

    fn discover_includes(&mut self) {
        let mut i = 0;
        while i < self.files.len() {
            let base = self.files[i].path.parent().map(Path::to_path_buf).unwrap_or_default();
            let mut found = Vec::new();
            if let Ok(kdl) = &self.files[i].kdl {
                for node in kdl.nodes().iter().filter(|n| n.name().value() == "include") {
                    if let Some(kdl::KdlValue::String(s)) = node.entries().first().map(|e| e.value()) {
                        found.push(resolve_include(&base, s));
                    }
                }
            }
            for path in found {
                if self.files.iter().any(|f| f.path == path) {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    self.files.push(SourceFile::new(path, text));
                }
            }
            i += 1;
        }
    }

    pub fn main(&self) -> &SourceFile {
        &self.files[0]
    }

    // ---- queries ----

    pub fn node(&self, r: &NodeRef) -> Option<&KdlNode> {
        let kdl = self.files.get(r.file)?.kdl.as_ref().ok()?;
        let (first, rest) = r.idx.split_first()?;
        let mut node = kdl.nodes().get(*first)?;
        for &i in rest {
            node = node.children()?.nodes().get(i)?;
        }
        Some(node)
    }

    /// Every node matching the path, in config order. `Seg::Name` matches every node of that
    /// name at its level; callers usually want the last one ([`Self::find`]).
    pub fn find_all(&self, path: &[Seg]) -> Vec<NodeRef> {
        self.find_all_in(None, path)
    }

    /// Like [`Self::find_all`], relative to the children of `base` (or the top level).
    pub fn find_all_in(&self, base: Option<&NodeRef>, path: &[Seg]) -> Vec<NodeRef> {
        let mut out = Vec::new();
        if let Some(base) = base {
            if let Some(children) = self.node(base).and_then(|n| n.children()) {
                collect(children, base, path, &mut out);
            }
            return out;
        }
        for (fi, file) in self.files.iter().enumerate() {
            if let Ok(kdl) = &file.kdl {
                let root = NodeRef { file: fi, idx: Vec::new() };
                collect(kdl, &root, path, &mut out);
            }
        }
        out
    }

    /// The node that takes effect for this path (the last match).
    pub fn find(&self, path: &[Seg]) -> Option<NodeRef> {
        self.find_in(None, path)
    }

    pub fn find_in(&self, base: Option<&NodeRef>, path: &[Seg]) -> Option<NodeRef> {
        self.find_all_in(base, path).pop()
    }

    pub fn get(&self, path: &[Seg]) -> Option<&KdlNode> {
        self.get_in(None, path)
    }

    pub fn get_in(&self, base: Option<&NodeRef>, path: &[Seg]) -> Option<&KdlNode> {
        self.find_in(base, path).and_then(|r| self.node(&r))
    }

    pub fn has(&self, path: &[Seg]) -> bool {
        self.find(path).is_some()
    }

    pub fn has_in(&self, base: Option<&NodeRef>, path: &[Seg]) -> bool {
        self.find_in(base, path).is_some()
    }

    pub fn args(&self, path: &[Seg]) -> Vec<Val> {
        self.args_in(None, path)
    }

    pub fn args_in(&self, base: Option<&NodeRef>, path: &[Seg]) -> Vec<Val> {
        self.get_in(base, path).map(node_args).unwrap_or_default()
    }

    pub fn arg(&self, path: &[Seg]) -> Option<Val> {
        self.args(path).into_iter().next()
    }

    pub fn arg_in(&self, base: Option<&NodeRef>, path: &[Seg]) -> Option<Val> {
        self.args_in(base, path).into_iter().next()
    }

    pub fn prop(&self, path: &[Seg], key: &str) -> Option<Val> {
        self.prop_in(None, path, key)
    }

    pub fn prop_in(&self, base: Option<&NodeRef>, path: &[Seg], key: &str) -> Option<Val> {
        self.get_in(base, path).and_then(|n| node_prop(n, key))
    }

    /// Children of the node at `r`, as refs.
    pub fn children(&self, r: &NodeRef) -> Vec<NodeRef> {
        let n = self.node(r).and_then(|n| n.children()).map_or(0, |c| c.nodes().len());
        (0..n).map(|i| r.child(i)).collect()
    }

    /// Source text of a node, as written.
    pub fn node_text(&self, r: &NodeRef) -> Option<&str> {
        let span = self.node(r)?.span();
        self.files[r.file].text.get(span.offset()..span.offset() + span.len())
    }

    // ---- edits ----

    /// Sets the node's arguments, keeping its properties and children. Creates it if missing.
    pub fn set_args(&mut self, path: &[Seg], args: &[Val]) -> Result<()> {
        self.set_args_in(None, path, args)
    }

    pub fn set_args_in(&mut self, base: Option<&NodeRef>, path: &[Seg], args: &[Val]) -> Result<()> {
        let r = self.ensure_in(base, path)?;
        let node = self.node(&r).context("node vanished")?;
        let props: Vec<String> = node
            .entries()
            .iter()
            .filter(|e| e.name().is_some())
            .map(|e| self.span_text(r.file, e.span().offset(), e.span().len()).to_owned())
            .collect();
        let entries = args.iter().map(Val::to_kdl).chain(props).collect::<Vec<_>>();
        self.replace_entries(&r, &entries, &edit_key(base, path))
    }

    pub fn set_arg(&mut self, path: &[Seg], v: impl Into<Val>) -> Result<()> {
        self.set_args(path, &[v.into()])
    }

    pub fn set_arg_in(&mut self, base: Option<&NodeRef>, path: &[Seg], v: impl Into<Val>) -> Result<()> {
        self.set_args_in(base, path, &[v.into()])
    }

    /// Sets (or with `None`, removes) a property, creating the node if needed.
    pub fn set_prop(&mut self, path: &[Seg], key: &str, v: Option<Val>) -> Result<()> {
        self.set_prop_in(None, path, key, v)
    }

    pub fn set_prop_in(&mut self, base: Option<&NodeRef>, path: &[Seg], key: &str, v: Option<Val>) -> Result<()> {
        if v.is_none() && !self.has_in(base, path) {
            return Ok(());
        }
        let r = self.ensure_in(base, path)?;
        let key_id = format!("{}@{key}", edit_key(base, path));
        self.set_prop_ref(&r, key, v, &key_id)
    }

    /// Sets or removes a property on a specific node. `undo_key` groups rapid edits.
    pub fn set_prop_ref(&mut self, r: &NodeRef, key: &str, v: Option<Val>, undo_key: &str) -> Result<()> {
        let r = r.clone();
        let node = self.node(&r).context("node vanished")?;
        let mut entries = Vec::new();
        let mut replaced = false;
        for e in node.entries() {
            match e.name() {
                Some(n) if n.value() == key => {
                    if let Some(v) = &v {
                        entries.push(format!("{}={}", value::ident(key), v.to_kdl()));
                    }
                    replaced = true;
                }
                _ => entries.push(self.span_text(r.file, e.span().offset(), e.span().len()).to_owned()),
            }
        }
        if !replaced {
            if let Some(v) = &v {
                entries.push(format!("{}={}", value::ident(key), v.to_kdl()));
            }
        }
        self.replace_entries(&r, &entries, undo_key)
    }

    /// For flag-style settings (`prefer-no-csd`, `off`): present = true.
    pub fn set_flag(&mut self, path: &[Seg], on: bool) -> Result<()> {
        self.set_flag_in(None, path, on)
    }

    pub fn set_flag_in(&mut self, base: Option<&NodeRef>, path: &[Seg], on: bool) -> Result<()> {
        if on {
            if !self.has_in(base, path) {
                self.ensure_in(base, path)?;
            }
            Ok(())
        } else {
            self.remove_all_in(base, path)
        }
    }

    /// Removes every node matching the path.
    pub fn remove_all(&mut self, path: &[Seg]) -> Result<()> {
        self.remove_all_in(None, path)
    }

    pub fn remove_all_in(&mut self, base: Option<&NodeRef>, path: &[Seg]) -> Result<()> {
        let refs = self.find_all_in(base, path);
        if refs.is_empty() {
            return Ok(());
        }
        self.checkpoint(&edit_key(base, path));
        // Remove back to front so earlier offsets stay valid.
        let mut ranges: Vec<(usize, Range<usize>)> =
            refs.iter().filter_map(|r| Some((r.file, self.removal_range(r)?))).collect();
        ranges.sort_by_key(|(f, r)| (*f, std::cmp::Reverse(r.start)));
        for (f, range) in ranges {
            self.files[f].text.replace_range(range, "");
        }
        self.after_edit_all();
        Ok(())
    }

    pub fn remove(&mut self, r: &NodeRef) -> Result<()> {
        let range = self.removal_range(r).context("node not found")?;
        self.checkpoint("remove");
        self.splice(r.file, range, "");
        Ok(())
    }

    /// Replaces a whole node (name, entries and children) with new source text.
    pub fn replace_node(&mut self, r: &NodeRef, text: &str) -> Result<()> {
        let span = self.node(r).context("node not found")?.span();
        let range = span.offset()..span.offset() + span.len();
        let indent = self.line_indent(r.file, range.start);
        let text = reindent(text, &indent);
        self.checkpoint(&format!("replace:{:?}", r));
        self.splice(r.file, range, &text);
        Ok(())
    }

    /// Appends a child node (given as source text) to `parent`, or to the top level of the main
    /// file when `parent` is `None`. Returns the new node's ref.
    pub fn append(&mut self, parent: Option<&NodeRef>, text: &str) -> Result<NodeRef> {
        self.checkpoint("append");
        self.append_inner(parent, text)
    }

    fn append_inner(&mut self, parent: Option<&NodeRef>, text: &str) -> Result<NodeRef> {
        let Some(parent) = parent else {
            let file = &self.files[0].text;
            let mut ins = String::new();
            if !file.is_empty() && !file.ends_with('\n') {
                ins.push('\n');
            }
            if !file.is_empty() && !file.ends_with("\n\n") {
                ins.push('\n');
            }
            ins.push_str(text);
            ins.push('\n');
            let at = file.len();
            self.splice(0, at..at, &ins);
            let kdl = self.files[0].kdl.as_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
            return Ok(NodeRef { file: 0, idx: vec![kdl.nodes().len() - 1] });
        };

        let node = self.node(parent).context("parent not found")?;
        let file = parent.file;
        let pspan = node.span();
        let pindent = self.line_indent(file, pspan.offset());
        let count = node.children().map_or(0, |c| c.nodes().len());

        let (at, ins) = match node.children() {
            Some(children) => {
                let cs = children.span();
                let (start, end) = (cs.offset(), cs.offset() + cs.len());
                let inner = &self.files[file].text[start..end];
                if let Some(nl) = inner.rfind('\n') {
                    // Multi-line block: new line just before the closing brace's line.
                    let indent = match children.nodes().first() {
                        Some(first) => self.line_indent(file, first.span().offset()),
                        None => format!("{pindent}{INDENT}"),
                    };
                    let mut ins = format!("{indent}{}\n", reindent(text, &indent));
                    // An empty block written as `{\n}` has the newline as the only content.
                    if inner[..nl].trim().is_empty() && !inner.starts_with('\n') {
                        ins.insert(0, '\n');
                    }
                    (start + nl + 1, ins)
                } else {
                    // Inline block like `{ spawn "a"; }`.
                    let trimmed = inner.trim_end();
                    let sep = if trimmed.trim().is_empty() || trimmed.ends_with(';') { " " } else { "; " };
                    (start + trimmed.len(), format!("{sep}{text};"))
                }
            }
            None => {
                let indent = format!("{pindent}{INDENT}");
                let at = pspan.offset() + pspan.len();
                (at, format!(" {{\n{indent}{}\n{pindent}}}", reindent(text, &indent)))
            }
        };
        self.splice(file, at..at, &ins);
        Ok(parent.child(count))
    }

    /// Returns the node for `path`, creating it (and missing parents) in the main file.
    pub fn ensure(&mut self, path: &[Seg]) -> Result<NodeRef> {
        self.ensure_in(None, path)
    }

    pub fn ensure_in(&mut self, base: Option<&NodeRef>, path: &[Seg]) -> Result<NodeRef> {
        if let Some(r) = self.find_in(base, path) {
            return Ok(r);
        }
        let (last, parent_path) = path.split_last().context("empty path")?;
        let parent = if parent_path.is_empty() {
            base.cloned()
        } else {
            Some(self.ensure_in(base, parent_path)?)
        };
        let text = match last {
            Seg::Keyed(name, key) => format!("{} {}", value::ident(name), value::quote(key)),
            seg => value::ident(seg.name()),
        };
        self.checkpoint(&edit_key(base, path));
        self.append_inner(parent.as_ref(), &text)
    }

    /// Swaps two sibling nodes (used to reorder rules and binds).
    pub fn swap(&mut self, a: &NodeRef, b: &NodeRef) -> Result<()> {
        anyhow::ensure!(a.file == b.file, "nodes are in different files");
        let sa = self.node(a).context("node not found")?.span();
        let sb = self.node(b).context("node not found")?.span();
        let (first, second) = if sa.offset() < sb.offset() { (sa, sb) } else { (sb, sa) };
        let r1 = first.offset()..first.offset() + first.len();
        let r2 = second.offset()..second.offset() + second.len();
        let t1 = self.files[a.file].text[r1.clone()].to_owned();
        let t2 = self.files[a.file].text[r2.clone()].to_owned();
        self.checkpoint("swap");
        let text = &mut self.files[a.file].text;
        text.replace_range(r2, &t1);
        text.replace_range(r1, &t2);
        self.after_edit(a.file);
        Ok(())
    }

    /// Replaces a file's whole text (the raw editor).
    pub fn set_text(&mut self, file: usize, text: String) {
        if self.files[file].text == text {
            return;
        }
        self.checkpoint(&format!("raw:{file}"));
        self.files[file].text = text;
        self.after_edit(file);
    }

    // ---- undo ----

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo.pop() {
            let cur = self.snapshot();
            self.redo.push(cur);
            self.restore(snap);
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo.pop() {
            let cur = self.snapshot();
            self.undo.push(cur);
            self.restore(snap);
        }
    }

    /// Ends undo coalescing, so the next edit starts a new undo step.
    pub fn break_coalescing(&mut self) {
        self.last_edit = None;
    }

    pub fn is_dirty(&self) -> bool {
        self.files.iter().any(SourceFile::is_dirty)
    }

    // ---- validation ----

    /// niri's verdict on the current (possibly unsaved) text.
    pub fn validation(&mut self) -> &Validation {
        if self.validation.is_none() {
            self.validation = Some(validate::validate(self));
        }
        self.validation.as_ref().unwrap()
    }

    // ---- internals ----

    fn snapshot(&self) -> Vec<String> {
        self.files.iter().map(|f| f.text.clone()).collect()
    }

    fn restore(&mut self, snap: Vec<String>) {
        for (f, t) in self.files.iter_mut().zip(snap) {
            f.text = t;
        }
        self.last_edit = None;
        self.after_edit_all();
    }

    /// Records an undo step before an edit, merging rapid edits to the same key.
    fn checkpoint(&mut self, key: &str) {
        let now = Instant::now();
        let merge = matches!(&self.last_edit, Some((k, t)) if k == key && now - *t < COALESCE);
        if !merge {
            self.undo.push(self.snapshot());
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = Some((key.to_owned(), now));
    }

    fn splice(&mut self, file: usize, range: Range<usize>, with: &str) {
        self.files[file].text.replace_range(range, with);
        self.after_edit(file);
    }

    fn after_edit(&mut self, file: usize) {
        let f = &mut self.files[file];
        f.kdl = parse(&f.text);
        self.generation += 1;
        self.validation = None;
    }

    fn after_edit_all(&mut self) {
        for i in 0..self.files.len() {
            self.after_edit(i);
        }
    }

    fn span_text(&self, file: usize, offset: usize, len: usize) -> &str {
        &self.files[file].text[offset..offset + len]
    }

    /// Rewrites a node's entries (everything between its name and its children).
    fn replace_entries(&mut self, r: &NodeRef, entries: &[String], key: &str) -> Result<()> {
        let node = self.node(r).context("node not found")?;
        let name = node.name().span();
        let start = name.offset() + name.len();
        let end = node
            .entries()
            .iter()
            .map(|e| e.span().offset() + e.span().len())
            .max()
            .unwrap_or(start);
        let mut text: String = entries.iter().map(|e| format!(" {e}")).collect();
        if entries.is_empty() && end == start {
            return Ok(());
        }
        // Keep whatever separated the old entries from a children block.
        if end == start && node.children().is_some() {
            let after = &self.files[r.file].text[start..];
            if !after.starts_with(' ') {
                text.push(' ');
            }
        }
        if self.files[r.file].text[start..end] == text {
            return Ok(());
        }
        self.checkpoint(key);
        self.splice(r.file, start..end, &text);
        Ok(())
    }

    /// Range to delete for a node: its whole line when it sits alone on one.
    fn removal_range(&self, r: &NodeRef) -> Option<Range<usize>> {
        let span = self.node(r)?.span();
        let text = &self.files[r.file].text;
        let (mut s, mut e) = (span.offset(), span.offset() + span.len());
        // Swallow a `;` terminator and same-line trailing comment.
        let rest = &text[e..];
        let line_end = rest.find('\n').map_or(text.len(), |i| e + i);
        let tail = text[e..line_end].trim_start().trim_start_matches(';').trim();
        let line_start = text[..s].rfind('\n').map_or(0, |i| i + 1);
        let alone = text[line_start..s].trim().is_empty() && (tail.is_empty() || tail.starts_with("//"));
        if alone {
            s = line_start;
            e = (line_end + 1).min(text.len());
        } else if text[e..].starts_with(';') {
            e += 1;
        }
        Some(s..e)
    }

    fn line_indent(&self, file: usize, offset: usize) -> String {
        let text = &self.files[file].text;
        let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
        text[line_start..offset].chars().take_while(|c| c.is_whitespace()).collect()
    }
}

fn edit_key(base: Option<&NodeRef>, path: &[Seg]) -> String {
    match base {
        Some(b) => format!("{}:{:?}/{}", b.file, b.idx, path::display(path)),
        None => path::display(path),
    }
}

fn collect(doc: &KdlDocument, at: &NodeRef, path: &[Seg], out: &mut Vec<NodeRef>) {
    let Some((seg, rest)) = path.split_first() else { return };
    let mut nth = 0;
    for (i, node) in doc.nodes().iter().enumerate() {
        if node.name().value() != seg.name() {
            continue;
        }
        let hit = match seg {
            Seg::Name(_) => true,
            Seg::Nth(_, n) => {
                nth += 1;
                nth - 1 == *n
            }
            Seg::Keyed(_, k) => matches!(
                node.entries().iter().find(|e| e.name().is_none()).map(|e| e.value()),
                Some(kdl::KdlValue::String(s)) if s == k
            ),
        };
        if !hit {
            continue;
        }
        let r = at.child(i);
        if rest.is_empty() {
            out.push(r);
        } else if let Some(children) = node.children() {
            collect(children, &r, rest, out);
        }
    }
}

pub fn node_args(n: &KdlNode) -> Vec<Val> {
    n.entries().iter().filter(|e| e.name().is_none()).map(|e| Val::from_kdl(e.value())).collect()
}

pub fn node_prop(n: &KdlNode, key: &str) -> Option<Val> {
    n.entries()
        .iter()
        .rev()
        .find(|e| e.name().is_some_and(|k| k.value() == key))
        .map(|e| Val::from_kdl(e.value()))
}

/// Indents continuation lines of a multi-line snippet to sit under `indent`.
fn reindent(text: &str, indent: &str) -> String {
    let mut lines = text.lines();
    let mut out = lines.next().unwrap_or_default().to_owned();
    for line in lines {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(indent);
        }
        out.push_str(line);
    }
    out
}

fn resolve_include(base: &Path, s: &str) -> PathBuf {
    let path = match s.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(s),
    };
    if path.is_absolute() { path } else { base.join(path) }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(test)]
mod tests;
