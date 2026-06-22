// Rust PoC for sony-camera-portal — proves the camera protocol + local proxy
// server work as a single static binary that can run on iSH (iOS).
//
// Mirrors the Go implementation: SSDP discover -> device description ->
// recursive ContentDirectory Browse -> typed photos -> a tiny_http server that
// proxies /api/list, /api/thumb/:id, /api/photo/:id and serves a minimal grid.
//
// Deliberately blocking + single-threaded (no tokio): the request loop runs one
// at a time, which suits iSH's emulated environment.

use std::collections::HashMap;
use std::io::Read;
use std::net::UdpSocket;
use std::time::Duration;

const UA: &str = "UPnP/1.0 DLNADOC/1.50 Sony";
const DEFAULT_DESC_PORT: u16 = 64321;

#[derive(Clone, Debug)]
struct Photo {
    id: String,
    name: String,
    date: String,
    thumb_url: String, // camera-side URL
    full_url: String,  // camera-side URL
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = 8080;
    let mut camera_host: Option<String> = None;
    let mut selftest: Option<String> = None;
    let mut mock: Option<usize> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                port = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(8080);
            }
            "--camera-host" => {
                i += 1;
                camera_host = args.get(i).cloned();
            }
            "--selftest" => {
                i += 1;
                selftest = args.get(i).cloned();
            }
            "--mock" => {
                i += 1;
                mock = args.get(i).and_then(|s| s.parse().ok());
            }
            other => eprintln!("ignoring unknown arg: {other}"),
        }
        i += 1;
    }

    if let Some(dir) = selftest {
        run_selftest(&dir);
        return;
    }

    let backend = if let Some(n) = mock {
        eprintln!("mock mode: serving {n} fake photos (no camera)");
        Backend::Mock(n)
    } else {
        // Resolve the device-description URL: explicit host, else SSDP discovery.
        let desc_url = match &camera_host {
            Some(h) => format!("http://{h}:{DEFAULT_DESC_PORT}/DmsDesc.xml"),
            None => match ssdp_discover(Duration::from_secs(5)) {
                Some(u) => u,
                None => {
                    eprintln!("no camera found via SSDP; pass --camera-host <ip> (e.g. 10.0.0.1)");
                    std::process::exit(1);
                }
            },
        };
        eprintln!("device description: {desc_url}");
        let (control_url, service_type) = match load_service(&desc_url) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("not connected to the camera: {e}");
                std::process::exit(1);
            }
        };
        eprintln!("ContentDirectory control: {control_url}");
        Backend::Camera { control_url, service_type }
    };

    serve(port, backend);
}

// Backend is what the server pulls photos/images from: a real camera, or
// synthetic mock data (for UI dev without a camera).
enum Backend {
    Camera { control_url: String, service_type: String },
    Mock(usize),
}

// ---------- HTTP helpers (ureq, http-only) ----------

fn http_get_bytes(url: &str) -> Result<(Vec<u8>, String), String> {
    let resp = ureq::get(url)
        .set("User-Agent", UA)
        .call()
        .map_err(|e| e.to_string())?;
    let content_type = resp
        .header("Content-Type")
        .unwrap_or("image/jpeg")
        .to_string();
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok((buf, content_type))
}

fn soap_browse(control_url: &str, service_type: &str, object_id: &str) -> Result<Vec<u8>, String> {
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body><u:Browse xmlns:u=\"{st}\"><ObjectID>{oid}</ObjectID>\
<BrowseFlag>BrowseDirectChildren</BrowseFlag><Filter>*</Filter>\
<StartingIndex>0</StartingIndex><RequestedCount>50</RequestedCount>\
<SortCriteria></SortCriteria></u:Browse></s:Body></s:Envelope>",
        st = service_type,
        oid = object_id
    );
    // Retry through Sony's transient 503s.
    let mut last = String::new();
    for attempt in 0..3 {
        match ureq::post(control_url)
            .set("Content-Type", "text/xml; charset=\"utf-8\"")
            .set("SOAPACTION", &format!("\"{service_type}#Browse\""))
            .set("User-Agent", UA)
            .send_string(&body)
        {
            Ok(resp) => {
                let mut buf = Vec::new();
                resp.into_reader()
                    .read_to_end(&mut buf)
                    .map_err(|e| e.to_string())?;
                return Ok(buf);
            }
            Err(e) => {
                last = e.to_string();
                std::thread::sleep(Duration::from_millis(400 * (attempt + 1)));
            }
        }
    }
    Err(last)
}

// ---------- Parsing (roxmltree) ----------

fn load_service(desc_url: &str) -> Result<(String, String), String> {
    let (bytes, _) = http_get_bytes(desc_url)?;
    let xml = String::from_utf8_lossy(&bytes);
    parse_device_description(&xml, desc_url)
}

fn parse_device_description(xml: &str, desc_url: &str) -> Result<(String, String), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    for service in doc.descendants().filter(|n| n.tag_name().name() == "service") {
        let mut stype = String::new();
        let mut ctrl = String::new();
        for c in service.children() {
            match c.tag_name().name() {
                "serviceType" => stype = c.text().unwrap_or("").trim().to_string(),
                "controlURL" => ctrl = c.text().unwrap_or("").trim().to_string(),
                _ => {}
            }
        }
        if stype.contains("ContentDirectory") && !ctrl.is_empty() {
            return Ok((resolve_url(desc_url, &ctrl), stype));
        }
    }
    Err("no ContentDirectory service in device description".into())
}

// resolve_url joins a possibly-relative controlURL against the description URL's
// scheme://host:port. Good enough for the absolute and root-relative cases the
// camera emits.
fn resolve_url(base: &str, reference: &str) -> String {
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return reference.to_string();
    }
    // base like http://10.0.0.1:64321/DmsDesc.xml -> origin http://10.0.0.1:64321
    let origin = match base.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.split('/').next().unwrap_or(rest);
            format!("{scheme}://{host}")
        }
        None => base.to_string(),
    };
    if reference.starts_with('/') {
        format!("{origin}{reference}")
    } else {
        format!("{origin}/{reference}")
    }
}

struct BrowseResult {
    items: Vec<Photo>,
    containers: Vec<String>,
    number_returned: usize,
    total_matches: usize,
}

fn parse_browse(soap_bytes: &[u8]) -> Result<BrowseResult, String> {
    let soap = String::from_utf8_lossy(soap_bytes);
    let doc = roxmltree::Document::parse(&soap).map_err(|e| e.to_string())?;

    let text_of = |name: &str| -> String {
        doc.descendants()
            .find(|n| n.tag_name().name() == name)
            .and_then(|n| n.text())
            .unwrap_or("")
            .to_string()
    };
    let number_returned = text_of("NumberReturned").trim().parse().unwrap_or(0);
    let total_matches = text_of("TotalMatches").trim().parse().unwrap_or(0);
    let didl = text_of("Result");

    let mut items = Vec::new();
    let mut containers = Vec::new();
    if !didl.trim().is_empty() {
        let ddoc = roxmltree::Document::parse(&didl).map_err(|e| e.to_string())?;
        for node in ddoc.root_element().children().filter(|n| n.is_element()) {
            match node.tag_name().name() {
                "container" => {
                    if let Some(id) = node.attribute("id") {
                        containers.push(id.to_string());
                    }
                }
                "item" => {
                    let id = node.attribute("id").unwrap_or("").to_string();
                    let mut name = String::new();
                    let mut date = String::new();
                    let mut res: Vec<(String, String)> = Vec::new(); // (protocolInfo, url)
                    for c in node.children().filter(|n| n.is_element()) {
                        match c.tag_name().name() {
                            "title" => name = c.text().unwrap_or("").trim().to_string(),
                            "date" => date = c.text().unwrap_or("").trim().to_string(),
                            "res" => {
                                let proto = c.attribute("protocolInfo").unwrap_or("").to_string();
                                let url = c.text().unwrap_or("").trim().to_string();
                                if !url.is_empty() {
                                    res.push((proto, url));
                                }
                            }
                            _ => {}
                        }
                    }
                    let (thumb, full) = select_urls(&res);
                    items.push(Photo { id, name, date, thumb_url: thumb, full_url: full });
                }
                _ => {}
            }
        }
    }
    Ok(BrowseResult { items, containers, number_returned, total_matches })
}

// dlna_pn extracts DLNA.ORG_PN from a protocolInfo string ("" if absent).
fn dlna_pn(protocol_info: &str) -> &str {
    const KEY: &str = "DLNA.ORG_PN=";
    if let Some(i) = protocol_info.find(KEY) {
        let v = &protocol_info[i + KEY.len()..];
        let end = v.find([';', ':']).unwrap_or(v.len());
        &v[..end]
    } else {
        ""
    }
}

// select_urls picks thumbnail (JPEG_TN, falling back) and full-res original
// (the PN-less res, falling back), by DLNA profile rather than position.
fn select_urls(res: &[(String, String)]) -> (String, String) {
    let mut by_pn: HashMap<&str, &str> = HashMap::new();
    let mut original = "";
    for (proto, url) in res {
        let pn = dlna_pn(proto);
        if pn.is_empty() {
            original = url;
        } else {
            by_pn.entry(pn).or_insert(url);
        }
    }
    let pick = |keys: &[&str]| -> String {
        for k in keys {
            if let Some(u) = by_pn.get(k) {
                return u.to_string();
            }
        }
        String::new()
    };
    let thumb = {
        let t = pick(&["JPEG_TN", "JPEG_SM", "JPEG_LRG"]);
        if t.is_empty() { original.to_string() } else { t }
    };
    let full = if !original.is_empty() {
        original.to_string()
    } else {
        pick(&["JPEG_LRG", "JPEG_SM", "JPEG_TN"])
    };
    (thumb, full)
}

// list_all recursively browses from the root, collecting all photos.
fn list_all(control_url: &str, service_type: &str) -> Result<Vec<Photo>, String> {
    let mut photos = Vec::new();
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut queue = vec!["0".to_string()];
    while let Some(id) = queue.pop() {
        if seen.contains_key(&id) {
            continue;
        }
        seen.insert(id.clone(), ());
        let raw = soap_browse(control_url, service_type, &id)?;
        let r = parse_browse(&raw)?;
        photos.extend(r.items);
        for c in r.containers {
            if !seen.contains_key(&c) {
                queue.push(c);
            }
        }
    }
    Ok(photos)
}

// ---------- SSDP discovery ----------

fn ssdp_discover(timeout: Duration) -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    for st in ["urn:schemas-upnp-org:device:MediaServer:1", "ssdp:all"] {
        let msg = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n\
MAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {st}\r\n\r\n"
        );
        let _ = sock.send_to(msg.as_bytes(), "239.255.255.250:1900");
    }
    let deadline = std::time::Instant::now() + timeout;
    let mut buf = [0u8; 65535];
    while std::time::Instant::now() < deadline {
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                for line in text.split("\r\n") {
                    if let Some((k, v)) = line.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("location") {
                            return Some(v.trim().to_string());
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }
    None
}

// ---------- HTTP server (tiny_http) ----------

fn serve(port: u16, backend: Backend) {
    let addr = format!("127.0.0.1:{port}");
    let server = match tiny_http::Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("listen on {addr}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("sony-camera-portal (rust PoC) listening at http://{addr}");

    let mut photos: HashMap<String, Photo> = HashMap::new();

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        if url == "/" || url == "/index.html" {
            let _ = request.respond(html_response(INDEX_HTML, "text/html; charset=utf-8"));
        } else if url == "/api/list" {
            let listed = match &backend {
                Backend::Camera { control_url, service_type } => list_all(control_url, service_type),
                Backend::Mock(n) => Ok((0..*n).map(mock_photo).collect()),
            };
            match listed {
                Ok(list) => {
                    photos = list.iter().map(|p| (p.id.clone(), p.clone())).collect();
                    let json = list_json(&list);
                    let _ = request.respond(html_response(&json, "application/json; charset=utf-8"));
                }
                Err(e) => {
                    let body = format!("{{\"error\":{}}}", json_string(&e));
                    let resp = tiny_http::Response::from_string(body)
                        .with_status_code(503)
                        .with_header(header("Content-Type", "application/json; charset=utf-8"));
                    let _ = request.respond(resp);
                }
            }
        } else if let Some(id) = url.strip_prefix("/api/thumb/") {
            serve_image(request, &backend, photos.get(id).cloned(), false);
        } else if let Some(id) = url.strip_prefix("/api/photo/") {
            serve_image(request, &backend, photos.get(id).cloned(), true);
        } else {
            let _ = request.respond(tiny_http::Response::from_string("not found").with_status_code(404));
        }
    }
}

// serve_image proxies a real camera image, or renders an SVG placeholder in mock
// mode. as_download marks the full-resolution route as an attachment.
fn serve_image(request: tiny_http::Request, backend: &Backend, photo: Option<Photo>, as_download: bool) {
    let Some(p) = photo else {
        let _ = request.respond(tiny_http::Response::from_string("not found").with_status_code(404));
        return;
    };
    match backend {
        Backend::Camera { .. } => {
            let url = if as_download { &p.full_url } else { &p.thumb_url };
            match http_get_bytes(url) {
                Ok((bytes, content_type)) => {
                    let mut resp = tiny_http::Response::from_data(bytes)
                        .with_header(header("Content-Type", &content_type));
                    if as_download {
                        resp = resp.with_header(header(
                            "Content-Disposition",
                            &format!("attachment; filename=\"{}\"", p.name),
                        ));
                    }
                    let _ = request.respond(resp);
                }
                Err(e) => {
                    let _ = request.respond(
                        tiny_http::Response::from_string(format!("upstream error: {e}"))
                            .with_status_code(502),
                    );
                }
            }
        }
        Backend::Mock(_) => {
            let resp = tiny_http::Response::from_string(svg_for(&p))
                .with_header(header("Content-Type", "image/svg+xml; charset=utf-8"));
            let _ = request.respond(resp);
        }
    }
}

// ---------- Mock data (UI dev without a camera) ----------

fn mock_photo(i: usize) -> Photo {
    Photo {
        id: format!("mock-{i:03}"),
        name: format!("DSC{:05}.JPG", 7000 + i),
        date: format!("2026-06-{:02}T{:02}:00:00", 1 + (i % 28), i % 24),
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
    let idx: usize = p.id.rsplit('-').next().and_then(|s| s.parse().ok()).unwrap_or(0);
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

fn header(field: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(field.as_bytes(), value.as_bytes()).unwrap()
}

fn html_response(body: &str, content_type: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(body).with_header(header("Content-Type", content_type))
}

// ---------- JSON (minimal, dependency-light via serde_json for escaping) ----------

fn json_string(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn list_json(photos: &[Photo]) -> String {
    let arr: Vec<serde_json::Value> = photos
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "date": p.date,
                "thumbUrl": format!("/api/thumb/{}", p.id),
                "fullUrl": format!("/api/photo/{}", p.id),
            })
        })
        .collect();
    serde_json::Value::Array(arr).to_string()
}

// ---------- Offline self-test against captured fixtures ----------

fn run_selftest(dir: &str) {
    let desc = std::fs::read_to_string(format!("{dir}/DmsDesc.xml")).expect("read DmsDesc.xml");
    let (ctrl, st) =
        parse_device_description(&desc, "http://10.0.0.1:64321/DmsDesc.xml").expect("parse desc");
    println!("controlURL  = {ctrl}");
    println!("serviceType = {st}");

    let browse = std::fs::read(format!("{dir}/browse_response.xml")).expect("read browse_response.xml");
    let r = parse_browse(&browse).expect("parse browse");
    println!(
        "browse: {} items, {} containers (NumberReturned={}, TotalMatches={})",
        r.items.len(),
        r.containers.len(),
        r.number_returned,
        r.total_matches
    );
    for p in r.items.iter().take(3) {
        println!("  - {} | {} | thumb={} | full={}", p.id, p.name, p.thumb_url, p.full_url);
    }
}

const INDEX_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Sony Camera Portal — Rust PoC</title>
<style>
body{font-family:-apple-system,sans-serif;background:#111;color:#eee;margin:0}
h3{padding:12px 12px 0}
.g{display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:8px;padding:12px}
figure{margin:0}
img{width:100%;aspect-ratio:3/2;object-fit:cover;border-radius:6px;background:#222}
div.n{font-size:11px;color:#aaa;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
</style></head><body>
<h3>Sony Camera Portal — Rust PoC</h3>
<div class="g" id="g">loading…</div>
<script>
fetch('/api/list').then(r=>r.json()).then(ps=>{
  document.getElementById('g').innerHTML = ps.map(p =>
    `<figure><img loading="lazy" src="${p.thumbUrl}"><div class="n">${p.name}</div></figure>`
  ).join('') || 'no photos';
}).catch(e => { document.getElementById('g').textContent = 'Error: ' + e; });
</script></body></html>"#;
