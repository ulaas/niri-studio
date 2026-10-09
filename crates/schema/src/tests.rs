use std::path::PathBuf;

use doc::{p, Document, Val};

use super::*;

fn check(text: &str, what: &str, failures: &mut Vec<String>) {
    let mut d = Document::from_text(PathBuf::from("/nonexistent/config.kdl"), text.to_owned());
    let v = d.validation();
    if !v.ok() {
        failures.push(format!("{what}: {}\n{text}", v.errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; ")));
    }
}

fn values(o: &Opt) -> Vec<Option<Val>> {
    match o.kind {
        Kind::Flag => vec![None],
        Kind::Bool => vec![Some(Val::Bool(true)), Some(Val::Bool(false))],
        Kind::Int { min, max } => vec![Some(Val::Int(min)), Some(Val::Int(max))],
        Kind::Float { min, max } => vec![Some(Val::Float(min)), Some(Val::Float(max))],
        Kind::Str | Kind::Path => vec![Some(Val::str("x"))],
        Kind::Enum(vs) => vs.iter().map(|v| Some(Val::str(*v))).collect(),
        Kind::Color => vec![Some(Val::str("#12345678"))],
    }
}

#[test]
fn every_option_is_accepted_by_niri() {
    let mut failures = Vec::new();
    for o in OPTIONS {
        for v in values(o) {
            let mut d = Document::from_text(PathBuf::from("/nonexistent/config.kdl"), String::new());
            match &v {
                None => d.set_flag(&p(o.path), true).unwrap(),
                Some(v) => d.set_arg(&p(o.path), v.clone()).unwrap(),
            }
            check(&d.main().text, &format!("{} = {v:?}", o.path), &mut failures);
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n\n"));
}

#[test]
fn every_action_is_accepted_by_niri() {
    let mut failures = Vec::new();
    for (name, hint) in ACTIONS {
        let arg = match *name {
            "spawn" | "spawn-sh" => " \"true\"",
            "set-window-height" | "set-column-width" => " \"+10%\"",
            "switch-layout" => " \"next\"",
            _ if !hint.is_empty() => " 1",
            _ => "",
        };
        let text = if matches!(*name, "next-window" | "previous-window") {
            format!("recent-windows {{\n    binds {{\n        Mod+F1 {{ {name}{arg}; }}\n    }}\n}}\n")
        } else {
            format!("binds {{\n    Mod+F1 {{ {name}{arg}; }}\n}}\n")
        };
        check(&text, name, &mut failures);
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n\n"));
}

#[test]
fn rule_props_are_accepted_by_niri() {
    let mut failures = Vec::new();
    for (node, props) in [("window-rule", WINDOW_RULE_PROPS), ("layer-rule", LAYER_RULE_PROPS)] {
        for (name, kind, _, _) in props {
            let o = Opt { path: name, kind: *kind, label: "", help: "", page: Page::Rules, section: "" };
            for v in values(&o) {
                let v = v.map(|v| v.to_kdl()).unwrap_or_default();
                check(&format!("{node} {{\n    {name} {v}\n}}\n"), &format!("{node}/{name}"), &mut failures);
            }
        }
    }
    for (flag, _) in WINDOW_MATCH_FLAGS {
        check(&format!("window-rule {{\n    match {flag}=true\n    exclude {flag}=false\n}}\n"), flag, &mut failures);
    }
    for (_, _, lines) in QUICK_RULES {
        let body: String = lines.iter().map(|l| format!("    {l}\n")).collect();
        check(&format!("window-rule {{\n    match app-id=\"^x$\"\n{body}}}\n"), &format!("{lines:?}"), &mut failures);
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n\n"));
}

#[test]
fn defaults_match_niri() {
    use doc::Document;
    let parse = |text: &str| {
        let mut d = Document::from_text(PathBuf::from("/nonexistent/config.kdl"), text.to_owned());
        d.validation().config.clone().unwrap_or_else(|| panic!("invalid: {text}"))
    };
    let empty = parse("");
    // Any `border { }` block turns the border on, so its values are compared with an empty block.
    let empty_border = parse("layout {\n    border {\n    }\n}\n");
    let mut failures = Vec::new();
    for (path, value) in DEFAULTS {
        let base = if path.starts_with("layout/border/") { &empty_border } else { &empty };
        let mut d = Document::from_text(PathBuf::from("/nonexistent/config.kdl"), String::new());
        // Render through the doc API so nesting is created the same way the app does it.
        d.ensure(&p(path)).unwrap();
        let text = d.main().text.replacen(path.rsplit('/').next().unwrap(), &format!("{} {value}", path.rsplit('/').next().unwrap()), 1);
        if *parse(&text) != **base {
            failures.push(format!("{path} {value} is not niri's default"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn defaults_refer_to_real_options() {
    for (path, ..) in DEFAULTS.iter().map(|(p, v)| (p, v, "")).chain(NOMINAL_DEFAULTS.iter().map(|(p, v, n)| (p, v, *n))) {
        let known = OPTIONS.iter().any(|o| o.path == *path) || path.starts_with("layout/focus-ring/") || path.starts_with("layout/border/") || path.starts_with("layout/shadow/");
        assert!(known, "{path} isn't in the schema");
    }
}

