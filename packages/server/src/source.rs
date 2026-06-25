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
        // The display label: the camera's friendlyName (e.g. "ILCE-6000"), or
        // the IP if it didn't advertise one.
        self.cam.label().to_string()
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
