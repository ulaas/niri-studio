//! KDL v1 value rendering.
//!
//! kdl-rs emits KDL v2 syntax (`#true`, `#null`) for values it formats itself, which niri's
//! knuffel-based parser rejects. Everything we write into the file is therefore rendered here,
//! by hand, as v1 text.

use std::fmt::Write as _;

/// A scalar value as written into the config.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl Val {
    pub fn str(s: impl Into<String>) -> Self {
        Val::Str(s.into())
    }

    /// Renders the value as KDL v1 source text.
    pub fn to_kdl(&self) -> String {
        match self {
            Val::Str(s) => quote(s),
            Val::Int(i) => i.to_string(),
            Val::Float(f) => float(*f),
            Val::Bool(b) => b.to_string(),
            Val::Null => "null".to_owned(),
        }
    }

    pub fn from_kdl(v: &kdl::KdlValue) -> Self {
        match v {
            kdl::KdlValue::String(s) => Val::Str(s.clone()),
            kdl::KdlValue::Integer(i) => Val::Int(*i as i64),
            kdl::KdlValue::Float(f) => Val::Float(*f),
            kdl::KdlValue::Bool(b) => Val::Bool(*b),
            kdl::KdlValue::Null => Val::Null,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Val::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Val::Int(i) => Some(*i as f64),
            Val::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Val::Int(i) => Some(*i),
            Val::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Val::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl From<&str> for Val {
    fn from(s: &str) -> Self {
        Val::Str(s.to_owned())
    }
}
impl From<String> for Val {
    fn from(s: String) -> Self {
        Val::Str(s)
    }
}
impl From<i64> for Val {
    fn from(i: i64) -> Self {
        Val::Int(i)
    }
}
impl From<f64> for Val {
    fn from(f: f64) -> Self {
        Val::Float(f)
    }
}
impl From<bool> for Val {
    fn from(b: bool) -> Self {
        Val::Bool(b)
    }
}

/// Floats always carry a decimal point so knuffel decodes them as floats.
pub fn float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{f:.1}")
    } else {
        // Shortest round-trip representation, trimmed of noise from slider arithmetic.
        let s = format!("{:.6}", f);
        let s = s.trim_end_matches('0');
        if s.ends_with('.') {
            format!("{s}0")
        } else {
            s.to_owned()
        }
    }
}

/// A KDL v1 quoted string.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A node name or property key: bare when KDL v1 allows it, quoted otherwise.
pub fn ident(s: &str) -> String {
    const NON_IDENT: &str = "\\/(){}<>;[]=,\"";
    let bare = !s.is_empty()
        && !matches!(s, "true" | "false" | "null")
        && !s.chars().any(|c| c.is_whitespace() || c.is_control() || NON_IDENT.contains(c))
        && !s.starts_with(|c: char| c.is_ascii_digit())
        && !(s.len() > 1
            && s.starts_with(['-', '+'])
            && s[1..].starts_with(|c: char| c.is_ascii_digit()));
    if bare {
        s.to_owned()
    } else {
        quote(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_v1() {
        assert_eq!(Val::Bool(true).to_kdl(), "true");
        assert_eq!(Val::Null.to_kdl(), "null");
        assert_eq!(Val::Float(1.0).to_kdl(), "1.0");
        assert_eq!(Val::Float(0.333333333).to_kdl(), "0.333333");
        assert_eq!(Val::str("/a/b \"c\"").to_kdl(), r#""/a/b \"c\"""#);
        assert_eq!(ident("Mod+Shift+Slash"), "Mod+Shift+Slash");
        assert_eq!(ident("has space"), "\"has space\"");
        assert_eq!(ident("1abc"), "\"1abc\"");
    }
}
