use std::path::PathBuf;

use super::*;

fn doc(text: &str) -> Document {
    Document::from_text(PathBuf::from("/nonexistent/config.kdl"), text.to_owned())
}

fn assert_valid(d: &mut Document) {
    let errors = d.validation().errors.clone();
    assert!(errors.is_empty(), "niri rejected:\n{}\n{errors:#?}", d.main().text);
}

const BASE: &str = "// my config\nlayout {\n    gaps 16 // keep me\n    focus-ring {\n        width 4\n    }\n}\n\nbinds {\n    Mod+T { spawn \"alacritty\"; }\n}\n";

#[test]
fn round_trips_real_configs() {
    let mut paths = vec![PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../niri/resources/default-config.kdl"))];
    paths.push(Document::default_path());
    for path in paths.into_iter().filter(|p| p.exists()) {
        let d = Document::load(&path).unwrap();
        for f in &d.files {
            let kdl = f.kdl.as_ref().unwrap_or_else(|e| panic!("{:?}: {e}", f.path));
            assert_eq!(kdl.to_string(), f.text, "{:?} did not round-trip", f.path);
        }
    }
}

#[test]
fn set_existing_arg_keeps_comment() {
    let mut d = doc(BASE);
    d.set_arg(&p("layout/gaps"), 24i64).unwrap();
    assert!(d.main().text.contains("    gaps 24 // keep me\n"), "{}", d.main().text);
    assert_valid(&mut d);
}

#[test]
fn creates_nested_nodes() {
    let mut d = doc(BASE);
    d.set_arg(&p("layout/border/width"), 2.5).unwrap();
    d.set_arg(&p("input/keyboard/xkb/layout"), "us,tr").unwrap();
    let t = &d.main().text;
    assert!(t.contains("    border {\n        width 2.5\n    }\n"), "{t}");
    assert!(t.contains("input {\n    keyboard {\n        xkb {\n            layout \"us,tr\"\n        }\n    }\n}\n"), "{t}");
    assert_valid(&mut d);
}

#[test]
fn flags_and_props() {
    let mut d = doc(BASE);
    d.set_flag(&p("prefer-no-csd"), true).unwrap();
    d.set_flag(&p("layout/focus-ring/off"), true).unwrap();
    assert!(d.has(&p("prefer-no-csd")));
    assert_valid(&mut d);
    d.set_flag(&p("layout/focus-ring/off"), false).unwrap();
    assert!(!d.main().text.contains("off"));

    let bind = vec![Seg::Name("binds".into()), Seg::Name("Mod+T".into())];
    d.set_prop(&bind, "allow-when-locked", Some(Val::Bool(true))).unwrap();
    assert!(d.main().text.contains("Mod+T allow-when-locked=true { spawn"), "{}", d.main().text);
    d.set_prop(&bind, "allow-when-locked", None).unwrap();
    assert!(d.main().text.contains("Mod+T { spawn"), "{}", d.main().text);
    assert_valid(&mut d);
}

#[test]
fn append_to_inline_block_and_remove() {
    let mut d = doc(BASE);
    let bind = d.find(&[Seg::Name("binds".into()), Seg::Name("Mod+T".into())]).unwrap();
    d.append(Some(&bind), "spawn \"foot\"").unwrap();
    assert!(d.main().text.contains("{ spawn \"alacritty\"; spawn \"foot\"; }"), "{}", d.main().text);
    d.remove(&bind).unwrap();
    assert!(d.main().text.contains("binds {\n}\n"), "{}", d.main().text);
    assert_valid(&mut d);
}

#[test]
fn keyed_nodes_and_swap() {
    let mut d = doc(BASE);
    let out = [Seg::Keyed("output".into(), "eDP-1".into()), Seg::Name("scale".into())];
    d.set_arg(&out, 1.5).unwrap();
    assert!(d.main().text.contains("output \"eDP-1\" {\n    scale 1.5\n}\n"), "{}", d.main().text);
    d.append(None, "window-rule {\n    match app-id=\"a\"\n}").unwrap();
    d.append(None, "window-rule {\n    match app-id=\"b\"\n}").unwrap();
    let rules = d.find_all(&p("window-rule"));
    d.swap(&rules[0], &rules[1]).unwrap();
    let t = &d.main().text;
    assert!(t.find("app-id=\"b\"").unwrap() < t.find("app-id=\"a\"").unwrap());
    assert_valid(&mut d);
}

#[test]
fn undo_coalesces_and_redo() {
    let mut d = doc(BASE);
    for g in 17..30i64 {
        d.set_arg(&p("layout/gaps"), g).unwrap();
    }
    d.break_coalescing();
    d.set_arg(&p("layout/focus-ring/width"), 8i64).unwrap();
    d.undo();
    assert!(d.main().text.contains("gaps 29"));
    d.undo();
    assert_eq!(d.main().text, BASE);
    d.redo();
    assert!(d.main().text.contains("gaps 29"));
}

#[test]
fn reports_errors_with_lines() {
    let mut d = doc("layout {\n    gaps \"wide\"\n}\n");
    let v = d.validation();
    assert!(!v.ok());
    assert_eq!(v.errors[0].line, Some(2), "{:#?}", v.errors);
}
