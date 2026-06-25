//! Localhost HTTP server: serves the embedded web UI and proxies `/api` to the
//! connected camera. The server starts WITHOUT a camera; the UI connects /
//! changes IP / reconnects at runtime via `/api/state` and `/api/connect`.
//!
//! Routing is a pure `handle()` function so it can be unit-tested without
//! binding a socket; `serve()` wires it to `tiny_http`.

#[cfg(feature = "mock")]
mod mock;
mod pager;
mod source;
mod state;

use std::sync::Arc;

pub use state::{AppState, StateInfo};

/// Supplies the embedded web bundle (implemented by the cli crate via rust-embed).
pub trait AssetSource: Send + Sync {
    /// Bytes + content-type for a normalized asset name ("index.html",
    /// "assets/app.js"), or None if absent.
    fn get(&self, name: &str) -> Option<(Vec<u8>, String)>;
}

/// A transport-agnostic HTTP response produced by `handle()`.
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    pub content_disposition: Option<String>,
    pub cache_control: Option<String>,
}

/// Proxied media is content-addressed by camera object id, so it's immutable —
/// let the browser cache it (a re-mounted virtualized tile won't re-hit the camera).
const MEDIA_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

impl Response {
    fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Response {
            status,
            content_type: content_type.to_string(),
            body,
            content_disposition: None,
            cache_control: None,
        }
    }
    fn json(status: u16, v: serde_json::Value) -> Self {
        Response::new(
            status,
            "application/json; charset=utf-8",
            v.to_string().into_bytes(),
        )
    }
    fn text(status: u16, s: impl Into<String>) -> Self {
        Response::new(status, "text/plain; charset=utf-8", s.into().into_bytes())
    }
    fn not_found() -> Self {
        Response::text(404, "not found")
    }
}

/// Route one request. Pure: no I/O beyond what the state/source perform.
pub fn handle(
    state: &AppState,
    assets: &dyn AssetSource,
    method: &str,
    raw_path: &str,
    body: &[u8],
) -> Response {
    let path = raw_path.split('?').next().unwrap_or(raw_path);
    match (method, path) {
        ("GET", "/api/state") => Response::json(200, state_json(&state.info())),

        ("POST", "/api/connect") => {
            let host = parse_connect_host(body);
            match state.connect(host.as_deref()) {
                Ok(info) => Response::json(200, state_json(&info)),
                // Keep the (untouched) current state; info carries last_error.
                Err(_) => Response::json(502, state_json(&state.info())),
            }
        }

        ("GET", "/api/list") => {
            let (offset, limit) = parse_page_params(raw_path);
            match state.list_page(offset, limit) {
                Ok(page) => Response::json(200, page_json(&page, state.source_gen())),
                Err(e) => Response::json(503, serde_json::json!({ "error": e })),
            }
        }

        ("GET", p) if p.starts_with("/api/thumb/") => {
            proxy(state, &p["/api/thumb/".len()..], false)
        }
        ("GET", p) if p.starts_with("/api/photo/") => proxy(state, &p["/api/photo/".len()..], true),

        ("GET", p) => serve_asset(assets, p),

        _ => Response::not_found(),
    }
}

fn proxy(state: &AppState, id: &str, full: bool) -> Response {
    let Some(photo) = state.photo(id) else {
        return Response::not_found();
    };
    let fetched = if full {
        state.fetch_full(&photo)
    } else {
        state.fetch_thumb(&photo)
    };
    match fetched {
        Ok((bytes, content_type)) => {
            let mut r = Response::new(200, &content_type, bytes);
            r.cache_control = Some(MEDIA_CACHE_CONTROL.to_string());
            if full {
                r.content_disposition = Some(format!("attachment; filename=\"{}\"", photo.name));
            }
            r
        }
        Err(e) => Response::text(502, format!("upstream error: {e}")),
    }
}

fn serve_asset(assets: &dyn AssetSource, path: &str) -> Response {
    let name = if path == "/" {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };
    match assets.get(name) {
        Some((bytes, content_type)) => Response::new(200, &content_type, bytes),
        None => Response::not_found(),
    }
}

fn parse_connect_host(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("host").and_then(|h| h.as_str()).map(str::to_string))
        .filter(|s| !s.trim().is_empty())
}

fn state_json(info: &StateInfo) -> serde_json::Value {
    serde_json::json!({
        "connected": info.connected,
        "host": info.host,
        "error": info.error,
        "photoCount": info.photo_count,
    })
}

/// Parse `?offset=&limit=` (defaults 0 / 60, limit clamped to [1, 500]).
fn parse_page_params(raw_path: &str) -> (usize, usize) {
    let mut offset = 0usize;
    let mut limit = 60usize;
    if let Some(query) = raw_path.split('?').nth(1) {
        for kv in query.split('&') {
            if let Some((k, v)) = kv.split_once('=') {
                match k {
                    "offset" => offset = v.parse().unwrap_or(0),
                    "limit" => limit = v.parse().unwrap_or(60),
                    _ => {}
                }
            }
        }
    }
    (offset, limit.clamp(1, 500))
}

fn page_json(page: &pager::Page, gen: u64) -> serde_json::Value {
    // `?v=<gen>` busts the (immutable) browser cache when the source changes;
    // the proxy ignores the query and resolves by id.
    let photos: Vec<serde_json::Value> = page
        .photos
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "date": p.date,
                "thumbUrl": format!("/api/thumb/{}?v={}", p.id, gen),
                "fullUrl": format!("/api/photo/{}?v={}", p.id, gen),
            })
        })
        .collect();
    serde_json::json!({
        "photos": photos,
        "total": page.total,
        "hasMore": page.has_more,
    })
}

/// Number of worker threads. Fixed and small so a stuck request (e.g. a slow
/// connect or an unresponsive read) never freezes the others, while staying
/// predictable under iSH's emulation (no unbounded thread-per-request spawn).
const WORKERS: usize = 4;

/// Bind `addr` (caller passes a loopback address) and serve requests
/// concurrently until the process ends. Requests are handled by a fixed worker
/// pool, so a slow `/api/connect` doesn't block `/api/state`, media, or assets.
pub fn serve(addr: &str, state: AppState, assets: Box<dyn AssetSource>) -> Result<(), String> {
    let server = Arc::new(tiny_http::Server::http(addr).map_err(|e| e.to_string())?);
    let state = Arc::new(state);
    let assets: Arc<dyn AssetSource> = Arc::from(assets);

    let mut workers = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let server = Arc::clone(&server);
        let state = Arc::clone(&state);
        let assets = Arc::clone(&assets);
        workers.push(std::thread::spawn(move || {
            while let Ok(request) = server.recv() {
                serve_one(request, &state, assets.as_ref());
            }
        }));
    }
    for w in workers {
        let _ = w.join();
    }
    Ok(())
}

/// Read one request, route it, and write the response.
fn serve_one(mut request: tiny_http::Request, state: &AppState, assets: &dyn AssetSource) {
    let method = match request.method() {
        tiny_http::Method::Get => "GET",
        tiny_http::Method::Post => "POST",
        _ => "OTHER",
    };
    let url = request.url().to_string();
    let mut body = Vec::new();
    if method == "POST" {
        let _ = request.as_reader().read_to_end(&mut body);
    }

    let resp = handle(state, assets, method, &url, &body);

    let mut http = tiny_http::Response::from_data(resp.body)
        .with_status_code(resp.status)
        .with_header(header("Content-Type", &resp.content_type));
    if let Some(cd) = resp.content_disposition {
        http = http.with_header(header("Content-Disposition", &cd));
    }
    if let Some(cc) = resp.cache_control {
        http = http.with_header(header("Cache-Control", &cc));
    }
    let _ = request.respond(http);
}

fn header(field: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(field.as_bytes(), value.as_bytes()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAssets;
    impl AssetSource for DummyAssets {
        fn get(&self, name: &str) -> Option<(Vec<u8>, String)> {
            if name == "index.html" {
                Some((b"<html>gallery</html>".to_vec(), "text/html".to_string()))
            } else {
                None
            }
        }
    }

    fn req(state: &AppState, method: &str, path: &str) -> Response {
        handle(state, &DummyAssets, method, path, b"")
    }

    #[test]
    fn state_disconnected() {
        let r = req(&AppState::new(), "GET", "/api/state");
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(v["connected"], false);
        assert!(v["host"].is_null());
    }

    #[test]
    fn list_disconnected_is_503() {
        let r = req(&AppState::new(), "GET", "/api/list");
        assert_eq!(r.status, 503);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert!(v["error"].is_string());
    }

    #[cfg(feature = "mock")]
    #[test]
    fn mock_list_has_proxied_urls() {
        let state = AppState::with_mock(3);
        let r = req(&state, "GET", "/api/list");
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        let arr = v["photos"].as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(v["total"], 3);
        assert_eq!(v["hasMore"], false);
        let id = arr[0]["id"].as_str().unwrap();
        // URLs carry a ?v=<source gen> cache-buster; the path resolves by id.
        assert!(arr[0]["thumbUrl"]
            .as_str()
            .unwrap()
            .starts_with(&format!("/api/thumb/{id}?v=")));
        assert!(arr[0]["fullUrl"]
            .as_str()
            .unwrap()
            .starts_with(&format!("/api/photo/{id}?v=")));
    }

    #[cfg(feature = "mock")]
    #[test]
    fn paginates_without_overlap() {
        let state = AppState::with_mock(10);
        let p1: serde_json::Value =
            serde_json::from_slice(&req(&state, "GET", "/api/list?offset=0&limit=4").body).unwrap();
        assert_eq!(p1["photos"].as_array().unwrap().len(), 4);
        assert_eq!(p1["total"], 10);
        assert_eq!(p1["hasMore"], true);

        let p_last: serde_json::Value =
            serde_json::from_slice(&req(&state, "GET", "/api/list?offset=8&limit=4").body).unwrap();
        assert_eq!(p_last["photos"].as_array().unwrap().len(), 2);
        assert_eq!(p_last["hasMore"], false);
        assert_ne!(p1["photos"][0]["id"], p_last["photos"][0]["id"]);
    }

    #[cfg(feature = "mock")]
    #[test]
    fn mock_thumb_is_svg_after_list() {
        let state = AppState::with_mock(2);
        let list: serde_json::Value =
            serde_json::from_slice(&req(&state, "GET", "/api/list").body).unwrap();
        let id = list["photos"][0]["id"].as_str().unwrap().to_string();
        let r = req(&state, "GET", &format!("/api/thumb/{id}"));
        assert_eq!(r.status, 200);
        assert!(r.content_type.contains("svg"));
    }

    #[cfg(feature = "mock")]
    #[test]
    fn photo_route_is_attachment() {
        let state = AppState::with_mock(1);
        let list: serde_json::Value =
            serde_json::from_slice(&req(&state, "GET", "/api/list").body).unwrap();
        let id = list["photos"][0]["id"].as_str().unwrap().to_string();
        let r = req(&state, "GET", &format!("/api/photo/{id}"));
        assert_eq!(r.status, 200);
        assert!(r
            .content_disposition
            .as_deref()
            .unwrap_or("")
            .contains("attachment"));
    }

    #[cfg(feature = "mock")]
    #[test]
    fn media_is_cacheable_but_listing_is_not() {
        let state = AppState::with_mock(1);
        let list: serde_json::Value =
            serde_json::from_slice(&req(&state, "GET", "/api/list").body).unwrap();
        let id = list["photos"][0]["id"].as_str().unwrap().to_string();

        let thumb = req(&state, "GET", &format!("/api/thumb/{id}"));
        assert!(thumb
            .cache_control
            .as_deref()
            .unwrap_or("")
            .contains("immutable"));
        let photo = req(&state, "GET", &format!("/api/photo/{id}"));
        assert!(photo
            .cache_control
            .as_deref()
            .unwrap_or("")
            .contains("immutable"));

        // The listing must NOT be cached (it changes as pages/cameras change).
        assert!(req(&state, "GET", "/api/list").cache_control.is_none());
    }

    #[cfg(feature = "mock")]
    #[test]
    fn unknown_id_404() {
        let state = AppState::with_mock(1);
        let _ = req(&state, "GET", "/api/list");
        assert_eq!(req(&state, "GET", "/api/thumb/nope").status, 404);
        assert_eq!(req(&state, "GET", "/api/photo/nope").status, 404);
    }

    #[test]
    fn serves_index() {
        let r = req(&AppState::new(), "GET", "/");
        assert_eq!(r.status, 200);
        assert!(String::from_utf8_lossy(&r.body).contains("gallery"));
    }
}
