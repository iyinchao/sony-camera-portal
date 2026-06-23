//! A photo Source backs the gallery: a real camera, or synthetic mock data for
//! UI dev without a camera. The server holds at most one connected Source.

use crate::pager::{Page, Pager};
use camera::{Camera, Photo};
use std::sync::Mutex;

pub trait Source: Send {
    /// A human label for the active source (e.g. the camera host).
    fn host(&self) -> String;
    /// Return photos `[offset, offset+limit)` plus paging metadata.
    fn list_page(&self, offset: usize, limit: usize) -> Result<Page, String>;
    fn fetch_thumb(&self, photo: &Photo) -> Result<(Vec<u8>, String), String>;
    fn fetch_full(&self, photo: &Photo) -> Result<(Vec<u8>, String), String>;
}

/// A real camera connected over its Wi-Fi AP, paginated lazily by a `Pager`.
pub struct RealCamera {
    cam: Camera,
    pager: Mutex<Pager>,
}

impl RealCamera {
    pub fn new(cam: Camera) -> Self {
        Self {
            cam,
            pager: Mutex::new(Pager::new()),
        }
    }
}

impl Source for RealCamera {
    fn host(&self) -> String {
        self.cam.host().to_string()
    }
    fn list_page(&self, offset: usize, limit: usize) -> Result<Page, String> {
        self.pager.lock().unwrap().page(&self.cam, offset, limit)
    }
    fn fetch_thumb(&self, photo: &Photo) -> Result<(Vec<u8>, String), String> {
        self.cam.fetch(&photo.thumb_url)
    }
    fn fetch_full(&self, photo: &Photo) -> Result<(Vec<u8>, String), String> {
        self.cam.fetch(&photo.full_url)
    }
}

/// Synthetic photos with SVG placeholder images (`--mock N`).
pub struct MockSource {
    photos: Vec<Photo>,
}

impl MockSource {
    pub fn new(n: usize) -> Self {
        Self {
            photos: (0..n).map(mock_photo).collect(),
        }
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
        Ok((
            svg_for(photo).into_bytes(),
            "image/svg+xml; charset=utf-8".to_string(),
        ))
    }
    fn fetch_full(&self, photo: &Photo) -> Result<(Vec<u8>, String), String> {
        self.fetch_thumb(photo)
    }
}

fn mock_photo(i: usize) -> Photo {
    Photo {
        id: format!("mock-{i:03}"),
        name: format!("DSC{:05}.JPG", 7000 + i),
        // ~6 per day so date grouping is visible in the UI.
        date: format!("2026-06-{:02}T{:02}:00:00", 1 + i / 6, i % 24),
        thumb_url: String::new(),
        full_url: String::new(),
    }
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
