//! Runtime connection state: which camera (if any) is connected, the cached
//! id→photo map, and the last error. Mutated at runtime by `/api/connect`.

use crate::pager::Page;
use crate::source::{RealCamera, Source};
use camera::{Camera, Photo};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[cfg(feature = "mock")]
use crate::mock::MockSource;
#[cfg(feature = "mock")]
use std::{path::PathBuf, time::Duration};

struct Inner {
    source: Option<Box<dyn Source>>,
    photos: HashMap<String, Photo>,
    last_error: Option<String>,
}

/// How `connect()` produces a Source: a real camera, or a mock (after an
/// optional simulated delay, so the connecting UX can be tested).
#[cfg(feature = "mock")]
enum Connector {
    Real,
    Mock { spec: MockSpec, delay: Duration },
}

/// What a mock connect builds.
#[cfg(feature = "mock")]
enum MockSpec {
    Synthetic(usize),
    /// A directory of images, and how many photos to produce (None = one per
    /// file; more than the file count cycles through them).
    Dir(PathBuf, Option<usize>),
}

/// Thread-safe app state, shared across the server's worker threads. `epoch`
/// orders concurrent connect attempts: each bumps it on entry, and only commits
/// its result if still current (so a stale attempt can't clobber a newer one).
pub struct AppState {
    inner: Mutex<Inner>,
    epoch: AtomicU64,
    /// Bumped whenever the active source is swapped. Media URLs carry it as
    /// `?v=<gen>` so the browser caches aggressively within a session but a new
    /// source (reconnect / different camera / mock swap) busts the cache —
    /// object ids are reused across sources, so the bytes can change.
    source_gen: AtomicU64,
    #[cfg(feature = "mock")]
    connector: Connector,
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
            // Seed from the wall clock so the cache-buster differs across process
            // restarts too (e.g. restarting with a different --mock source), not
            // just across in-process reconnects.
            source_gen: AtomicU64::new(now_millis()),
            #[cfg(feature = "mock")]
            connector: Connector::Real,
        }
    }

    /// Start already "connected" to synthetic mock data (used by tests).
    #[cfg(feature = "mock")]
    pub fn with_mock(n: usize) -> Self {
        let state = Self::new();
        state.inner.lock().unwrap().source = Some(Box::new(MockSource::new(n)));
        state.source_gen.fetch_add(1, Ordering::SeqCst);
        state
    }

    /// The current source generation (changes when the active source is swapped).
    pub fn source_gen(&self) -> u64 {
        self.source_gen.load(Ordering::SeqCst)
    }

    /// Start DISCONNECTED with a mock connector: the web UI drives the connect
    /// (auto-discover) like a real camera, but it resolves to `n` synthetic
    /// photos after `delay_secs` (so the connecting UX can be exercised).
    #[cfg(feature = "mock")]
    pub fn mock_synthetic(n: usize, delay_secs: u64) -> Self {
        Self::with_connector(Connector::Mock {
            spec: MockSpec::Synthetic(n),
            delay: Duration::from_secs(delay_secs),
        })
    }

    /// Like `mock_synthetic`, but photos are backed by the image files in `dir`
    /// (`count` photos, cycling through the files if it exceeds the file count;
    /// None = one per file).
    #[cfg(feature = "mock")]
    pub fn mock_dir(dir: PathBuf, count: Option<usize>, delay_secs: u64) -> Self {
        Self::with_connector(Connector::Mock {
            spec: MockSpec::Dir(dir, count),
            delay: Duration::from_secs(delay_secs),
        })
    }

    #[cfg(feature = "mock")]
    fn with_connector(connector: Connector) -> Self {
        let mut state = Self::new();
        state.connector = connector;
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
        // Produce the source (no lock held during this potentially-slow work).
        let result = self.open_source(host);
        self.commit_connect(my_epoch, result)
    }

    /// Real-only build: connect to the host, or auto-discover.
    #[cfg(not(feature = "mock"))]
    fn open_source(&self, host: Option<&str>) -> Result<Box<dyn Source>, String> {
        real_connect(host)
    }

    /// Real build, or a mock (after a simulated delay) when configured.
    #[cfg(feature = "mock")]
    fn open_source(&self, host: Option<&str>) -> Result<Box<dyn Source>, String> {
        match &self.connector {
            Connector::Real => real_connect(host),
            Connector::Mock { spec, delay } => {
                std::thread::sleep(*delay);
                build_mock(spec).map(|m| Box::new(m) as Box<dyn Source>)
            }
        }
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
        result: Result<Box<dyn Source>, String>,
    ) -> Result<StateInfo, String> {
        let mut inner = self.inner.lock().unwrap();
        if self.epoch.load(Ordering::SeqCst) != my_epoch {
            return Err("superseded by a newer connect".into());
        }
        match result {
            Ok(source) => {
                inner.source = Some(source);
                inner.photos.clear();
                inner.last_error = None;
                drop(inner);
                self.source_gen.fetch_add(1, Ordering::SeqCst); // bust media cache
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

/// Wall-clock milliseconds since the epoch, used to seed the media cache-buster
/// so it's distinct across process restarts.
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Connect to a real camera (a typed host, or auto-discover) as a boxed Source.
fn real_connect(host: Option<&str>) -> Result<Box<dyn Source>, String> {
    match host {
        Some(h) => Camera::connect(h),
        None => Camera::discover(),
    }
    .map(|cam| Box::new(RealCamera::new(cam)) as Box<dyn Source>)
}

#[cfg(feature = "mock")]
fn build_mock(spec: &MockSpec) -> Result<MockSource, String> {
    match spec {
        MockSpec::Synthetic(n) => Ok(MockSource::new(*n)),
        MockSpec::Dir(p, count) => MockSource::from_dir(p, *count),
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
        let r = state.commit_connect(e, Err::<Box<dyn Source>, String>("boom".into()));
        assert_eq!(r.unwrap_err(), "boom");
        assert_eq!(state.info().error.as_deref(), Some("boom"));
    }

    #[test]
    fn superseded_connect_is_discarded() {
        let state = AppState::new();
        let e1 = state.begin_connect();
        let _e2 = state.begin_connect(); // a newer attempt started

        // The older attempt (e1) finishes last → superseded → touches nothing.
        let r = state.commit_connect(e1, Err::<Box<dyn Source>, String>("stale".into()));
        assert_eq!(r.unwrap_err(), "superseded by a newer connect");
        assert!(
            state.info().error.is_none(),
            "stale error must not be recorded"
        );

        // The newer attempt commits normally.
        let r2 = state.commit_connect(_e2, Err::<Box<dyn Source>, String>("fresh".into()));
        assert_eq!(r2.unwrap_err(), "fresh");
        assert_eq!(state.info().error.as_deref(), Some("fresh"));
    }
}
