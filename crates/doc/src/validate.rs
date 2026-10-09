//! Validation with niri's own config parser.

use std::ops::Range;

use miette::Diagnostic;

use crate::Document;

/// One problem reported by niri.
#[derive(Debug, Clone)]
pub struct Diag {
    pub message: String,
    pub help: Option<String>,
    /// Index into `Document::files`, when the error could be attributed to a file.
    pub file: Option<usize>,
    /// Byte range in that file, plus the label niri attached to it.
    pub span: Option<Range<usize>>,
    pub label: Option<String>,
    /// 1-based line number of `span.start`.
    pub line: Option<usize>,
}

pub struct Validation {
    pub errors: Vec<Diag>,
    /// The parsed config when valid; drives previews with exactly niri's interpretation.
    pub config: Option<Box<niri_config::Config>>,
    /// Included files that have unsaved edits; niri reads includes from disk, so their
    /// in-memory contents are not part of this verdict.
    pub unsaved_includes: bool,
}

impl Validation {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate(doc: &Document) -> Validation {
    let main = doc.main();
    let res = niri_config::Config::parse(&main.path, &main.text);
    let unsaved_includes = doc.files.iter().skip(1).any(|f| f.is_dirty());
    match res.config {
        Ok(config) => Validation { errors: Vec::new(), config: Some(Box::new(config)), unsaved_includes },
        Err(err) => {
            let mut errors = Vec::new();
            collect(doc, &err.main, &mut errors);
            for inc in &err.includes {
                collect(doc, inc, &mut errors);
            }
            if errors.is_empty() {
                errors.push(Diag {
                    message: err.to_string(),
                    help: None,
                    file: Some(0),
                    span: None,
                    label: None,
                    line: None,
                });
            }
            Validation { errors, config: None, unsaved_includes }
        }
    }
}

fn collect(doc: &Document, err: &knuffel::Error, out: &mut Vec<Diag>) {
    let src = err.source_code();
    let related: Vec<&dyn Diagnostic> = err.related().map(|r| r.collect()).unwrap_or_default();
    let list: Vec<&dyn Diagnostic> = if related.is_empty() { vec![err as &dyn Diagnostic] } else { related };

    for d in list {
        let label = d.labels().and_then(|mut l| l.next());
        let span = label.as_ref().map(|l| l.offset()..l.offset() + l.len());
        let file = match (src, &span) {
            (Some(src), Some(span)) => src
                .read_span(&(span.start, 0).into(), 0, 0)
                .ok()
                .and_then(|c| c.name().map(str::to_owned))
                .and_then(|name| file_index(doc, &name)),
            _ => None,
        }
        .or(Some(0));
        let line = match (file, &span) {
            (Some(f), Some(s)) => doc.files[f].text.get(..s.start).map(|t| t.matches('\n').count() + 1),
            _ => None,
        };
        out.push(Diag {
            message: d.to_string(),
            help: d.help().map(|h| h.to_string()),
            file,
            span,
            label: label.and_then(|l| l.label().map(str::to_owned)),
            line,
        });
    }
}

fn file_index(doc: &Document, name: &str) -> Option<usize> {
    doc.files.iter().position(|f| {
        f.path.to_string_lossy() == name
            || f.path.file_name().is_some_and(|n| n.to_string_lossy() == name)
    })
}
