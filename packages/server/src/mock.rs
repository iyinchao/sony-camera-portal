//! Mock photo Source for UI dev without a camera (feature `mock`, dev-only).
//! `--mock N` synthetic SVG placeholders, or `--mock-dir <path>` real image
//! files from a directory (cycling to N). Stripped from release builds
//! (`--no-default-features`).

use crate::pager::Page;
use crate::source::Source;
use camera::Photo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Mock photos. For directory-backed photos, `files` maps id → on-disk path;
/// synthetic photos leave it empty and render gradient SVGs.
pub struct MockSource {
    photos: Vec<Photo>,
    files: HashMap<String, PathBuf>,
}

impl MockSource {
    /// `n` synthetic photos with gradient SVG placeholders.
    pub fn new(n: usize) -> Self {
        let dates = generate_dates(n);
        let photos = (0..n)
            .map(|i| Photo {
                id: format!("mock-{i:03}"),
                name: format!("DSC{:05}.JPG", 7000 + i),
                date: dates[i].clone(),
                thumb_url: String::new(),
                full_url: String::new(),
            })
            .collect();
        Self {
            photos,
            files: HashMap::new(),
        }
    }

    /// Photos backed by the image files in `dir` (top level, common formats).
    /// `count` photos are produced (default: one per file); if `count` exceeds
    /// the number of files, the files are cycled through. Dates are spread by
    /// index so date grouping still demos nicely regardless of the files'
    /// timestamps; the image bytes come from disk.
    pub fn from_dir(dir: &Path, count: Option<usize>) -> Result<Self, String> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| format!("read dir {}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && is_image(p))
            .collect();
        if paths.is_empty() {
            return Err(format!("no image files in {}", dir.display()));
        }
        paths.sort();
        let n = count.unwrap_or(paths.len());
        let dates = generate_dates(n);
        let mut photos = Vec::with_capacity(n);
        let mut files = HashMap::new();
        for i in 0..n {
            let path = &paths[i % paths.len()]; // cycle through the files
            let id = format!("mock-{i:03}");
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("image")
                .to_string();
            photos.push(Photo {
                id: id.clone(),
                name,
                date: dates[i].clone(),
                thumb_url: String::new(),
                full_url: String::new(),
            });
            files.insert(id, path.clone());
        }
        Ok(Self { photos, files })
    }
}

impl Source for MockSource {
    fn host(&self) -> String {
        format!("mock ({} photos)", self.photos.len())
    }
    fn list_page(&self, offset: usize, limit: usize) -> Result<Page, String> {
        let total = self.photos.len();
        let end = (offset + limit).min(total);
        let photos = if offset < total {
            self.photos[offset..end].to_vec()
        } else {
            Vec::new()
        };
        Ok(Page {
            photos,
            total: Some(total),
            has_more: end < total,
        })
    }
    fn fetch_thumb(&self, photo: &Photo) -> Result<(Vec<u8>, String), String> {
        // Directory-backed: serve the real file (same for thumb and full, so you
        // see the actual aspect ratio / content). Synthetic: a gradient SVG.
        match self.files.get(&photo.id) {
            Some(path) => {
                let bytes =
                    std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
                Ok((bytes, content_type_for(path)))
            }
            None => Ok((
                svg_for(photo).into_bytes(),
                "image/svg+xml; charset=utf-8".to_string(),
            )),
        }
    }
    fn fetch_full(&self, photo: &Photo) -> Result<(Vec<u8>, String), String> {
        self.fetch_thumb(photo)
    }
}

/// Generate `n` ascending dates whose per-day group sizes are *random and
/// uneven*, sized to the total: the biggest day is up to 100 photos, but no more
/// than ~30% of `n` for small sets. The sequence is randomized once per call
/// (seeded from the clock), so each mock session gets a fresh distribution;
/// dates ascend with the index so newest/oldest paging stays correct.
fn generate_dates(n: usize) -> Vec<String> {
    if n == 0 {
        return Vec::new();
    }
    let max_day = ((n as f64 * 0.30) as usize).clamp(1, 100);
    let mut rng = Rng::seeded();
    let mut dates = Vec::with_capacity(n);
    let mut day_index = 0usize;
    let mut produced = 0usize;
    while produced < n {
        let size = (1 + rng.below(max_day)).min(n - produced); // 1..=max_day
        for pos in 0..size {
            dates.push(date_for(day_index, pos));
        }
        produced += size;
        day_index += 1;
    }
    dates
}

/// A valid calendar date for `day_index`, with `pos` (0-based within the day)
/// mapped to a distinct ascending time (supports up to ~100/day).
fn date_for(day_index: usize, pos: usize) -> String {
    let day = 1 + day_index % 28;
    let month = 1 + (day_index / 28) % 12;
    let year = 2026 + day_index / (28 * 12);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:00",
        (pos / 60) % 24,
        pos % 60
    )
}

/// Tiny deterministic xorshift PRNG, seeded from the wall clock.
struct Rng(u64);

impl Rng {
    fn seeded() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let mut r = Rng(nanos | 1);
        for _ in 0..4 {
            r.next_u64(); // warm up so close seeds diverge
        }
        r
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// A value in `0..n` (n must be > 0).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif", "bmp"];

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn content_type_for(p: &Path) -> String {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn palette(i: usize) -> (&'static str, &'static str) {
    const C: [(&str, &str); 6] = [
        ("#e74c3c", "#7d2820"),
        ("#e67e22", "#7a4212"),
        ("#f1c40f", "#7d6607"),
        ("#2ecc71", "#176b3a"),
        ("#3498db", "#1a5074"),
        ("#9b59b6", "#512f60"),
    ];
    C[i % 6]
}

fn svg_for(p: &Photo) -> String {
    let idx: usize =
        p.id.rsplit('-')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
    let (a, b) = palette(idx);
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='320' height='213' viewBox='0 0 320 213'>\
<defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'>\
<stop offset='0' stop-color='{a}'/><stop offset='1' stop-color='{b}'/></linearGradient></defs>\
<rect width='320' height='213' fill='url(#g)'/>\
<text x='12' y='200' font-family='sans-serif' font-size='15' fill='rgba(255,255,255,.9)'>{name}</text>\
</svg>",
        name = p.name
    )
}
