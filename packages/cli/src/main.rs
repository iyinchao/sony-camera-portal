//! sony-camera-portal CLI: parse flags, embed the web bundle, start the
//! localhost server. The server starts WITHOUT a camera — connect / change IP /
//! reconnect from the web UI.

use rust_embed::RustEmbed;
use server::{AppState, AssetSource};
#[cfg(feature = "mock")]
use std::path::PathBuf;

/// The built React frontend, embedded at compile time (debug builds read from
/// disk so `npm run dev` / rebuilds reflect without recompiling the binary).
#[derive(RustEmbed)]
#[folder = "../web/dist"]
struct WebAssets;

struct EmbeddedAssets;

impl AssetSource for EmbeddedAssets {
    fn get(&self, name: &str) -> Option<(Vec<u8>, String)> {
        let file = WebAssets::get(name)?;
        Some((file.data.into_owned(), content_type_for(name)))
    }
}

fn content_type_for(name: &str) -> String {
    let ct = match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    };
    ct.to_string()
}

struct Args {
    port: u16,
    no_open: bool,
    /// `--mock <N>`: number of mock photos. With `--mock-dir`, images come from
    /// that directory (cycling if N exceeds the file count); without it they're
    /// synthetic. `--mock-dir` alone uses every image in the directory once.
    /// (Mock support is the `mock` feature — stripped from release builds.)
    #[cfg(feature = "mock")]
    mock: Option<usize>,
    #[cfg(feature = "mock")]
    mock_dir: Option<PathBuf>,
    #[cfg(feature = "mock")]
    mock_delay: u64,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let mut a = Args {
        port: 8080,
        no_open: false,
        #[cfg(feature = "mock")]
        mock: None,
        #[cfg(feature = "mock")]
        mock_dir: None,
        #[cfg(feature = "mock")]
        mock_delay: 0,
    };
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--port" => {
                i += 1;
                a.port = argv.get(i).and_then(|s| s.parse().ok()).unwrap_or(a.port);
            }
            "--no-open" => a.no_open = true,
            #[cfg(feature = "mock")]
            "--mock" => {
                i += 1;
                a.mock = argv.get(i).and_then(|s| s.parse().ok());
            }
            #[cfg(feature = "mock")]
            "--mock-dir" => {
                i += 1;
                a.mock_dir = argv.get(i).map(PathBuf::from);
            }
            #[cfg(feature = "mock")]
            "--mock-delay" => {
                i += 1;
                a.mock_delay = argv.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            other => eprintln!("ignoring unknown arg: {other}"),
        }
        i += 1;
    }
    a
}

/// Build the initial app state. With the `mock` feature, `--mock`/`--mock-dir`
/// select a mock source; otherwise (release) the server always starts cameraless.
#[cfg(feature = "mock")]
fn build_state(args: &Args) -> AppState {
    match (&args.mock_dir, args.mock) {
        (Some(dir), count) => {
            let n = count.map_or_else(|| "all".to_string(), |n| n.to_string());
            eprintln!(
                "mock mode: {n} images from {} (connect delay {}s)",
                dir.display(),
                args.mock_delay
            );
            AppState::mock_dir(dir.clone(), count, args.mock_delay)
        }
        (None, Some(n)) => {
            eprintln!(
                "mock mode: {n} synthetic photos (connect delay {}s)",
                args.mock_delay
            );
            AppState::mock_synthetic(n, args.mock_delay)
        }
        (None, None) => AppState::new(),
    }
}

#[cfg(not(feature = "mock"))]
fn build_state(_args: &Args) -> AppState {
    AppState::new()
}

fn main() {
    let args = parse_args();

    // The server always starts; a camera is optional. Mock modes start
    // DISCONNECTED with a mock connector so the web UI drives the connect (and
    // `--mock-delay` simulates discovery latency) just like a real camera.
    let state = build_state(&args);

    // Bind 127.0.0.1 only — never expose the camera proxy to the LAN.
    let addr = format!("127.0.0.1:{}", args.port);
    let url = format!("http://{addr}");
    eprintln!("sony-camera-portal listening at {url}");

    if !args.no_open {
        let u = url.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            open_browser(&u);
        });
    }

    if let Err(e) = server::serve(&addr, state, Box::new(EmbeddedAssets)) {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}

/// Best-effort open of the default browser on desktop platforms. On headless
/// environments (iSH) this just fails silently; the printed URL is the entry.
fn open_browser(url: &str) {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("rundll32", vec!["url.dll,FileProtocolHandler", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = std::process::Command::new(cmd).args(args).spawn();
}
