//! Addressing nodes in a config.

use std::fmt;

/// One step of a node path.
#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    /// The last node with this name (niri lets later nodes override earlier ones).
    Name(String),
    /// The n-th (0-based) node with this name.
    Nth(String, usize),
    /// The node with this name whose first argument is this string, e.g. `output "eDP-1"`.
    Keyed(String, String),
}

impl Seg {
    pub fn name(&self) -> &str {
        match self {
            Seg::Name(n) | Seg::Nth(n, _) | Seg::Keyed(n, _) => n,
        }
    }
}

impl fmt::Display for Seg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Seg::Name(n) => write!(f, "{n}"),
            Seg::Nth(n, i) => write!(f, "{n}[{i}]"),
            Seg::Keyed(n, k) => write!(f, "{n}[{k:?}]"),
        }
    }
}

/// Builds a path from `/`-separated names: `p("layout/focus-ring/width")`.
pub fn p(path: &str) -> Vec<Seg> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| Seg::Name(s.to_owned()))
        .collect()
}

pub fn display(path: &[Seg]) -> String {
    path.iter().map(Seg::to_string).collect::<Vec<_>>().join("/")
}
