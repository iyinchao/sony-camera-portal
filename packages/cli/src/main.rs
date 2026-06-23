//! sony-camera-portal CLI: parse flags, embed the web bundle, start the
//! localhost server. The server starts WITHOUT a camera — connect / change IP /
//! reconnect from the web UI.

use rust_embed::RustEmbed;
use server::{AppState, AssetSource};

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
    mock: Option<usize>,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let mut a = Args {
        port: 8080,
        no_open: false,
        mock: None,
    };
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--port" => {
                i += 1;
                a.port = argv.get(i).and_then(|s| s.parse().ok()).unwrap_or(a.port);
            }
            "--no-open" => a.no_open = true,
            "--mock" => {
                i += 1;
                a.mock = argv.get(i).and_then(|s| s.parse().ok());
            }
            other => eprintln!("ignoring unknown arg: {other}"),
        }
        i += 1;
    }
    a
}

fn main() {
    let args = parse_args();

    // Build state. The server always starts; a camera is optional.
    let state = match args.mock {
        Some(n) => {
            eprintln!("mock mode: serving {n} fake photos (no camera)");
            AppState::with_mock(n)
        }
        // No camera at startup — the web UI connects / discovers / sets the IP.
        None => AppState::new(),
    };

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
