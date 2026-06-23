//! Runtime connection state: which camera (if any) is connected, the cached
//! id→photo map, and the last error. Mutated at runtime by `/api/connect`.

use crate::pager::Page;
use crate::source::{MockSource, RealCamera, Source};
use camera::{Camera, Photo};
use std::collections::HashMap;
use std::sync::Mutex;

struct Inner {
    source: Option<Box<dyn Source>>,
    photos: HashMap<String, Photo>,
    last_error: Option<String>,
}

/// Thread-safe app state. The server is single-threaded today, but the Mutex
/// keeps this sound if that changes.
pub struct AppState {
    inner: Mutex<Inner>,
}

/// A snapshot of connection state for `/api/state` and `/api/connect`.
pub struct StateInfo {
    pub connected: bool,
    pub host: Option<String>,
    pub error: Option<String>,
    pub photo_count: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                source: None,
                photos: HashMap::new(),
                last_error: None,
            }),
        }
    }

    /// Start already "connected" to synthetic mock data.
    pub fn with_mock(n: usize) -> Self {
        let state = Self::new();
        state.inner.lock().unwrap().source = Some(Box::new(MockSource::new(n)));
        state
    }

    pub fn info(&self) -> StateInfo {
        let inner = self.inner.lock().unwrap();
        StateInfo {
            connected: inner.source.is_some(),
            host: inner.source.as_ref().map(|s| s.host()),
            error: inner.last_error.clone(),
            photo_count: inner.photos.len(),
        }
    }

    /// Connect to `host` (or auto-discover when None). Validate-then-swap: the
    /// new camera is validated (its description fetched/parsed) before it
    /// replaces the active source, so a bad request never drops a good
    /// connection.
    pub fn connect(&self, host: Option<&str>) -> Result<StateInfo, String> {
        // A host (from the web UI) connects to that IP; otherwise auto-discover.
        let result = match host {
            Some(h) => Camera::connect(h),
            None => Camera::discover(),
        };
        match result {
            Ok(cam) => {
                let mut inner = self.inner.lock().unwrap();
                inner.source = Some(Box::new(RealCamera::new(cam)));
                inner.photos.clear();
                inner.last_error = None;
                drop(inner);
                Ok(self.info())
            }
            Err(e) => {
                self.inner.lock().unwrap().last_error = Some(e.clone());
                Err(e)
            }
        }
    }

    /// List a page of photos via the current source, merging them into the
    /// id→photo cache (so the thumb/photo proxy can resolve ids across pages).
    pub fn list_page(&self, offset: usize, limit: usize) -> Result<Page, String> {
        let mut inner = self.inner.lock().unwrap();
        let page = match inner.source.as_ref() {
            Some(src) => src.list_page(offset, limit)?,
            None => return Err("not connected".into()),
        };
        for p in &page.photos {
            inner.photos.insert(p.id.clone(), p.clone());
        }
        Ok(page)
    }

    pub fn photo(&self, id: &str) -> Option<Photo> {
        self.inner.lock().unwrap().photos.get(id).cloned()
    }

    pub fn fetch_thumb(&self, photo: &Photo) -> Result<(Vec<u8>, String), String> {
        let inner = self.inner.lock().unwrap();
        let src = inner.source.as_ref().ok_or("not connected")?;
        src.fetch_thumb(photo)
    }

    pub fn fetch_full(&self, photo: &Photo) -> Result<(Vec<u8>, String), String> {
        let inner = self.inner.lock().unwrap();
        let src = inner.source.as_ref().ok_or("not connected")?;
        src.fetch_full(photo)
    }
}
