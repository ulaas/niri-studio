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
