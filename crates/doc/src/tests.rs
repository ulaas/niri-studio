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
    // niri's default config (a copy of resources/default-config.kdl from the pinned niri commit),
    // and yours when there is one.
    let mut paths = vec![PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/niri-default-config.kdl"))];
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

#[test]
fn slashdashed_nodes_are_ignored() {
    let mut d = doc("/-window-rule {\n    match app-id=\"a\"\n}\nwindow-rule {\n    match app-id=\"b\"\n}\n");
    let rules = d.find_all(&p("window-rule"));
    assert_eq!(rules.len(), 1);
    assert_eq!(d.node_text(&rules[0]).unwrap(), "window-rule {\n    match app-id=\"b\"\n}");
    d.remove(&rules[0]).unwrap();
    assert_eq!(d.main().text, "/-window-rule {\n    match app-id=\"a\"\n}\n");
}

#[test]
fn labels_and_disabling() {
    let mut d = doc("window-rule {\n    match app-id=\"a\"\n}\n\n// Float pip\n/-window-rule {\n    match title=\"}\"\n}\n");
    let r = d.find(&p("window-rule")).unwrap();
    assert_eq!(d.label(&r), None);
    d.set_label(&r, Some("Terminals")).unwrap();
    assert!(d.main().text.starts_with("// Terminals\nwindow-rule {"), "{}", d.main().text);
    let r = d.find(&p("window-rule")).unwrap();
    assert_eq!(d.label(&r).as_deref(), Some("Terminals"));

    let dis = d.disabled_nodes("window-rule");
    assert_eq!(dis.len(), 1);
    assert_eq!(dis[0].body, "window-rule {\n    match title=\"}\"\n}");
    assert_eq!(d.label_at(0, dis[0].range.start).as_deref(), Some("Float pip"));

    d.enable(&dis[0]);
    assert_eq!(d.find_all(&p("window-rule")).len(), 2);
    let first = d.find_all(&p("window-rule"))[0].clone();
    d.disable(&first).unwrap();
    assert_eq!(d.find_all(&p("window-rule")).len(), 1);
    assert_valid(&mut d);

    let dis = d.disabled_nodes("window-rule");
    d.remove_disabled(&dis[0]);
    assert_eq!(d.main().text, "\n// Float pip\nwindow-rule {\n    match title=\"}\"\n}\n");
}

#[test]
fn swap_blocks_moves_labels() {
    let mut d = doc("// A\nwindow-rule {\n    match app-id=\"a\"\n}\n// B\nwindow-rule {\n    match app-id=\"b\"\n}\n");
    let rules = d.find_all(&p("window-rule"));
    d.swap_blocks(&rules[0], &rules[1]).unwrap();
    assert_eq!(d.main().text, "// B\nwindow-rule {\n    match app-id=\"b\"\n}\n// A\nwindow-rule {\n    match app-id=\"a\"\n}\n");
}

#[test]
fn comment_paragraphs_are_not_labels() {
    let d = doc("// This explains\n// the rule below.\nwindow-rule {\n}\n\n// Named\nwindow-rule {\n}\n");
    let rules = d.find_all(&p("window-rule"));
    assert_eq!(d.label(&rules[0]), None);
    assert_eq!(d.label(&rules[1]).as_deref(), Some("Named"));
}

#[test]
fn save_backs_up_and_restores() {
    let dir = std::env::temp_dir().join(format!("niri-studio-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Backups go under XDG_STATE_HOME; point it at the temp dir.
    std::env::set_var("XDG_STATE_HOME", dir.join("state"));
    let path = dir.join("config.kdl");
    std::fs::write(&path, BASE).unwrap();

    let mut d = Document::load(&path).unwrap();
    d.set_arg(&p("layout/gaps"), 4i64).unwrap();
    assert!(d.is_dirty());
    let out = d.save().unwrap();
    assert!(!d.is_dirty());
    assert!(std::fs::read_to_string(&path).unwrap().contains("gaps 4"));
    assert_eq!(out.backups.len(), 1);
    assert_eq!(out.backups[0].read().unwrap(), BASE);
    assert!(crate::list_backups().iter().any(|b| b.path == out.backups[0].path));

    d.restore_backups(&out.backups).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), BASE);
    assert!(d.changed_on_disk().is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}
