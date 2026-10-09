//! Saving, backups and external-change detection.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};

use crate::{home, Document};

const BACKUP_KEEP: usize = 100;

pub struct SaveOutcome {
    /// Backups taken of the previous on-disk contents, one per written file.
    pub backups: Vec<Backup>,
}

#[derive(Debug, Clone)]
pub struct Backup {
    pub path: PathBuf,
    /// The config file this is a backup of.
    pub of: PathBuf,
    /// Unix seconds.
    pub time: u64,
}

impl Backup {
    pub fn read(&self) -> Result<String> {
        fs::read_to_string(&self.path).with_context(|| format!("reading {:?}", self.path))
    }
}

pub fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".local/state"))
        .join("niri-studio")
}

fn backup_dir() -> PathBuf {
    state_dir().join("backups")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Writes `text` to `path` atomically, following a symlinked config to its real file.
pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = target.parent().context("config path has no parent")?;
    let name = target.file_name().context("config path has no file name")?.to_string_lossy();
    let tmp = dir.join(format!(".{name}.niri-studio-{}", std::process::id()));
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("creating {tmp:?}"))?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    if let Ok(meta) = fs::metadata(&target) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }
    fs::rename(&tmp, &target).with_context(|| format!("replacing {target:?}"))?;
    Ok(())
}

fn take_backup(of: &Path) -> Result<Option<Backup>> {
    let Ok(current) = fs::read_to_string(of) else { return Ok(None) };
    let dir = backup_dir();
    fs::create_dir_all(&dir).with_context(|| format!("creating {dir:?}"))?;
    let time = now();
    let name = of.file_name().map_or("config.kdl".into(), |n| n.to_string_lossy());
    let mut path = dir.join(format!("{time}-{name}"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{time}.{n}-{name}"));
        n += 1;
    }
    fs::write(&path, current)?;
    fs::write(path.with_extension("origin"), of.to_string_lossy().as_bytes())?;
    prune();
    Ok(Some(Backup { path, of: of.to_path_buf(), time }))
}

fn prune() {
    let mut all = list_backups();
    if all.len() > BACKUP_KEEP {
        all.sort_by_key(|b| b.time);
        for b in &all[..all.len() - BACKUP_KEEP] {
            let _ = fs::remove_file(&b.path);
            let _ = fs::remove_file(b.path.with_extension("origin"));
        }
    }
}

/// All backups, newest first.
pub fn list_backups() -> Vec<Backup> {
    let Ok(rd) = fs::read_dir(backup_dir()) else { return Vec::new() };
    let mut out: Vec<Backup> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "kdl"))
        .filter_map(|path| {
            let stem = path.file_name()?.to_str()?;
            let time = stem.split(['-', '.']).next()?.parse().ok()?;
            let of = fs::read_to_string(path.with_extension("origin")).ok()?.into();
            Some(Backup { path, of, time })
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse((b.time, b.path.clone())));
    out
}

impl Document {
    /// Writes every modified file, backing up what was on disk first.
    pub fn save(&mut self) -> Result<SaveOutcome> {
        let mut backups = Vec::new();
        for f in self.files.iter_mut().filter(|f| f.is_dirty()) {
            if let Some(b) = take_backup(&f.path)? {
                backups.push(b);
            }
            write_atomic(&f.path, &f.text)?;
            f.saved = f.text.clone();
        }
        Ok(SaveOutcome { backups })
    }

    /// Restores files on disk from backups (used to roll back a config niri rejected).
    /// The in-memory text is left alone, so the user can keep fixing it.
    pub fn restore_backups(&mut self, backups: &[Backup]) -> Result<()> {
        for b in backups {
            let text = b.read()?;
            write_atomic(&b.of, &text)?;
            if let Some(f) = self.files.iter_mut().find(|f| f.path == b.of) {
                f.saved = text;
            }
        }
        Ok(())
    }

    /// Files whose on-disk contents changed behind our back (e.g. edited in a text editor).
    pub fn changed_on_disk(&self) -> Vec<usize> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, f)| fs::read_to_string(&f.path).is_ok_and(|t| t != f.saved))
            .map(|(i, _)| i)
            .collect()
    }

    /// Treats `disk` as the file's saved state, so a newer on-disk version stops being reported
    /// as changed and the in-memory text counts as unsaved changes on top of it.
    pub fn mark_saved_as(&mut self, file: usize, disk: String) {
        self.files[file].saved = disk;
    }

    /// Reloads everything from disk, discarding unsaved changes (undoable).
    pub fn reload(&mut self) -> Result<()> {
        let fresh = Document::load(&self.files[0].path)?;
        self.checkpoint("reload");
        self.files = fresh.files;
        self.after_edit_all();
        Ok(())
    }
}

/// Formats unix seconds as local-ish `YYYY-MM-DD HH:MM:SS` (UTC; no tz database needed).
pub fn format_time(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC", rem / 3600, rem % 3600 / 60, rem % 60)
}
