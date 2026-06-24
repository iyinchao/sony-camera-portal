//! Runtime connection state: which camera (if any) is connected, the cached
//! id→photo map, and the last error. Mutated at runtime by `/api/connect`.

use crate::pager::Page;
use crate::source::{MockSource, RealCamera, Source};
use camera::{Camera, Photo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

struct Inner {
    source: Option<Box<dyn Source>>,
    photos: HashMap<String, Photo>,
    last_error: Option<String>,
}

/// Thread-safe app state, shared across the server's worker threads. `epoch`
/// orders concurrent connect attempts: each bumps it on entry, and only commits
/// its result if still current (so a stale attempt can't clobber a newer one).
pub struct AppState {
    inner: Mutex<Inner>,
    epoch: AtomicU64,
}

/// A snapshot of connection state for `/api/state` and `/api/connect`.
#[derive(Debug)]
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
            epoch: AtomicU64::new(0),
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
    /// connection. The slow discovery runs WITHOUT holding the lock, so other
    /// requests are served concurrently. An `epoch` makes a newer connect
    /// supersede an older in-flight one: if a newer attempt started while this
    /// one was working, this one's result is discarded instead of committed.
    pub fn connect(&self, host: Option<&str>) -> Result<StateInfo, String> {
        let my_epoch = self.begin_connect();
        // A host (from the web UI) connects to that IP; otherwise auto-discover.
        // (No lock held during this potentially-slow network work.)
        let result = match host {
            Some(h) => Camera::connect(h),
            None => Camera::discover(),
        };
        self.commit_connect(my_epoch, result)
    }

    /// Claim a connect epoch. The newest claimant has the highest epoch.
    fn begin_connect(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Commit a connect's result, but only if no newer connect has started since
    /// `my_epoch` was claimed; otherwise discard it (touch nothing).
    fn commit_connect(
        &self,
        my_epoch: u64,
        result: Result<Camera, String>,
    ) -> Result<StateInfo, String> {
        let mut inner = self.inner.lock().unwrap();
        if self.epoch.load(Ordering::SeqCst) != my_epoch {
            return Err("superseded by a newer connect".into());
        }
        match result {
            Ok(cam) => {
                inner.source = Some(Box::new(RealCamera::new(cam)));
                inner.photos.clear();
                inner.last_error = None;
                drop(inner);
                Ok(self.info())
            }
            Err(e) => {
                inner.last_error = Some(e.clone());
                drop(inner);
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

#[cfg(test)]
mod tests {
    use super::*;

    // The Err path exercises the epoch gate without needing a live Camera; the
    // Ok path (committing a source) goes through the same gate.

    #[test]
    fn fresh_connect_records_its_error() {
        let state = AppState::new();
        let e = state.begin_connect();
        let r = state.commit_connect(e, Err::<Camera, String>("boom".into()));
        assert_eq!(r.unwrap_err(), "boom");
        assert_eq!(state.info().error.as_deref(), Some("boom"));
    }

    #[test]
    fn superseded_connect_is_discarded() {
        let state = AppState::new();
        let e1 = state.begin_connect();
        let _e2 = state.begin_connect(); // a newer attempt started

        // The older attempt (e1) finishes last → superseded → touches nothing.
        let r = state.commit_connect(e1, Err::<Camera, String>("stale".into()));
        assert_eq!(r.unwrap_err(), "superseded by a newer connect");
        assert!(
            state.info().error.is_none(),
            "stale error must not be recorded"
        );

        // The newer attempt commits normally.
        let r2 = state.commit_connect(_e2, Err::<Camera, String>("fresh".into()));
        assert_eq!(r2.unwrap_err(), "fresh");
        assert_eq!(state.info().error.as_deref(), Some("fresh"));
    }
}
