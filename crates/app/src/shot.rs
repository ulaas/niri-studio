//! Developer aid: `NIRI_STUDIO_SHOT=Layout=/tmp/x.png niri-studio` renders that page, saves a
//! screenshot of the window and exits. Used to check the UI without a screenshot tool.
//! `NIRI_STUDIO_SEARCH=query` opens the search palette and `NIRI_STUDIO_APPEARANCE=1` the
//! appearance panel for the shot.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use egui::ColorImage;
use schema::Page;

pub struct Shot {
    pub page: Page,
    path: PathBuf,
    frames: u32,
    requested: bool,
}

impl Shot {
    pub fn from_env() -> Option<Self> {
        let spec = std::env::var("NIRI_STUDIO_SHOT").ok()?;
        let (page, path) = spec.split_once('=')?;
        let page = Page::ALL.into_iter().find(|p| format!("{p:?}").eq_ignore_ascii_case(page))?;
        Some(Self { page, path: path.into(), frames: 0, requested: false })
    }

    /// Call once per frame. Returns true when done and the app should close.
    pub fn tick(&mut self, ctx: &egui::Context) -> bool {
        self.frames += 1;
        ctx.request_repaint();
        let image: Option<Arc<ColorImage>> = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            if let Err(e) = write_png(&self.path, &image) {
                eprintln!("writing screenshot: {e}");
            }
            return true;
        }
        // Give layout, fonts and the IPC thread a moment to settle.
        if self.frames > 30 && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        false
    }
}

/// A tiny uncompressed PNG writer (stored deflate blocks), enough for screenshots.
fn write_png(path: &std::path::Path, img: &ColorImage) -> std::io::Result<()> {
    let [w, h] = img.size;
    let mut raw = Vec::with_capacity((w * 4 + 1) * h);
    for y in 0..h {
        raw.push(0);
        for p in &img.pixels[y * w..(y + 1) * w] {
            raw.extend_from_slice(&p.to_srgba_unmultiplied());
        }
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    std::fs::File::create(path)?.write_all(&out)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}
