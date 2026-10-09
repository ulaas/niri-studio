//! The system's XKB catalogue (layouts, variants, options) from `rules/evdev.lst`.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub code: String,
    /// Empty for the base layout.
    pub variant: String,
    pub description: String,
}

#[derive(Default)]
pub struct Catalog {
    /// Base layouts followed by variants, in file order.
    pub layouts: Vec<Layout>,
    /// `grp:*` options: (name, description).
    pub switchers: Vec<(String, String)>,
}

impl Catalog {
    pub fn load() -> Self {
        let root = std::env::var_os("XKB_CONFIG_ROOT").map(PathBuf::from).unwrap_or_else(|| "/usr/share/X11/xkb".into());
        ["evdev.lst", "base.lst"]
            .iter()
            .find_map(|f| std::fs::read_to_string(root.join("rules").join(f)).ok())
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        let mut cat = Catalog::default();
        let mut section = "";
        for line in text.lines() {
            if let Some(s) = line.strip_prefix("! ") {
                section = match s.trim() {
                    "layout" => "layout",
                    "variant" => "variant",
                    "option" => "option",
                    _ => "",
                };
                continue;
            }
            let line = line.trim();
            let Some((name, rest)) = line.split_once(char::is_whitespace) else { continue };
            let rest = rest.trim();
            match section {
                "layout" => cat.layouts.push(Layout { code: name.into(), variant: String::new(), description: rest.into() }),
                "variant" => {
                    if let Some((layout, desc)) = rest.split_once(':') {
                        cat.layouts.push(Layout { code: layout.trim().into(), variant: name.into(), description: desc.trim().into() });
                    }
                }
                "option" if name.starts_with("grp:") => cat.switchers.push((name.into(), rest.into())),
                _ => {}
            }
        }
        cat
    }

    pub fn describe(&self, code: &str, variant: &str) -> Option<&str> {
        self.layouts.iter().find(|l| l.code == code && l.variant == variant).map(|l| l.description.as_str())
    }

    /// Finds the layout niri reports by its description (e.g. "English (US)").
    pub fn by_description(&self, desc: &str) -> Option<&Layout> {
        self.layouts.iter().find(|l| l.description == desc)
    }

    pub fn search(&self, q: &str) -> Vec<&Layout> {
        let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
        self.layouts
            .iter()
            .filter(|l| {
                let hay = format!("{} {} {}", l.code, l.variant, l.description).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }
}

/// Splits a comma-separated XKB list, keeping empty entries (variants align by position).
pub fn split(s: &str) -> Vec<String> {
    if s.trim().is_empty() {
        Vec::new()
    } else {
        s.split(',').map(|p| p.trim().to_owned()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LST: &str = "! model\n  pc105  Generic 105-key PC\n\n! layout\n  us              English (US)\n  tr              Turkish\n\n! variant\n  intl            us: English (US, intl., with dead keys)\n  f               tr: Turkish (F)\n\n! option\n  grp                  Switching to another layout\n  grp:alt_shift_toggle Alt+Shift\n  caps:none            Caps Lock is disabled\n";

    #[test]
    fn parses_lst() {
        let c = Catalog::parse(LST);
        assert_eq!(c.layouts.len(), 4);
        assert_eq!(c.describe("tr", "f"), Some("Turkish (F)"));
        assert_eq!(c.by_description("English (US)").unwrap().code, "us");
        assert_eq!(c.switchers, vec![("grp:alt_shift_toggle".to_owned(), "Alt+Shift".to_owned())]);
        assert_eq!(c.search("turk f").len(), 1);
    }

    #[test]
    fn system_catalog_loads() {
        let c = Catalog::load();
        if std::path::Path::new("/usr/share/X11/xkb/rules/evdev.lst").exists() {
            assert!(c.layouts.len() > 100);
            assert!(c.switchers.iter().any(|(n, _)| n == "grp:alt_shift_toggle"));
        }
    }

    #[test]
    fn split_keeps_positions() {
        assert_eq!(split("us,tr"), vec!["us", "tr"]);
        assert_eq!(split(",f"), vec!["", "f"]);
        assert!(split("").is_empty());
    }
}
