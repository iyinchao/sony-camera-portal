// Rust PoC for sony-camera-portal — proves the camera protocol + local proxy
// server work as a single static binary that can run on iSH (iOS).
//
// Mirrors the Go implementation: SSDP discover -> device description ->
// recursive ContentDirectory Browse -> typed photos -> a tiny_http server that
// proxies /api/list, /api/thumb/:id, /api/photo/:id and serves a minimal grid.
//
// Deliberately blocking + single-threaded (no tokio): the request loop runs one
// at a time, which suits iSH's emulated environment.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

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
            None => match discover() {
                Some(u) => u,
                None => {
                    eprintln!("no camera found (SSDP + gateway probe); pass --camera-host <ip> (e.g. 10.0.0.1)");
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

// ---------- Minimal blocking HTTP client (iSH-safe) ----------
//
// No socket options (timeouts/non-blocking) are set — those are what made ureq
// fail with EINVAL on iSH. HTTP/1.0 + "Connection: close" lets us read the body
// to EOF without parsing Content-Length or chunked encoding.

fn split_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url.strip_prefix("http://").ok_or("only http:// is supported")?;
    let slash = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..slash];
    let path = if slash < rest.len() { &rest[slash..] } else { "/" };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().map_err(|_| "bad port")?),
        None => (authority.to_string(), 80u16),
    };
    Ok((host, port, path.to_string()))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

// http_request performs one blocking HTTP/1.0 request and returns
// (status, body, content_type).
fn http_request(
    method: &str,
    url: &str,
    extra_headers: &[(&str, &str)],
    body: Option<&[u8]>,
) -> Result<(u16, Vec<u8>, String), String> {
    let (host, port, path) = split_url(url)?;
    let mut stream =
        TcpStream::connect((host.as_str(), port)).map_err(|e| format!("connect {host}:{port}: {e}"))?;

    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: {host}:{port}\r\n");
    for (k, v) in extra_headers {
        req.push_str(k);
        req.push_str(": ");
        req.push_str(v);
        req.push_str("\r\n");
    }
    if let Some(b) = body {
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    req.push_str("Connection: close\r\n\r\n");

    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    if let Some(b) = body {
        stream.write_all(b).map_err(|e| e.to_string())?;
    }
    stream.flush().ok();

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;

    let sep = find_subslice(&raw, b"\r\n\r\n").ok_or("malformed HTTP response")?;
    let head = String::from_utf8_lossy(&raw[..sep]).into_owned();
    let body_bytes = raw[sep + 4..].to_vec();

    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0u16);
    let mut content_type = String::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-type") {
                content_type = v.trim().to_string();
            }
        }
    }
    Ok((status, body_bytes, content_type))
}

fn http_get_bytes(url: &str) -> Result<(Vec<u8>, String), String> {
    let (status, body, ct) = http_request("GET", url, &[("User-Agent", UA)], None)?;
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    Ok((body, if ct.is_empty() { "image/jpeg".to_string() } else { ct }))
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
    let soapaction = format!("\"{service_type}#Browse\"");
    let headers = [
        ("User-Agent", UA),
        ("Content-Type", "text/xml; charset=\"utf-8\""),
        ("SOAPACTION", soapaction.as_str()),
    ];
    // Retry through Sony's transient 503s.
    let mut last = String::new();
    for attempt in 0..3 {
        match http_request("POST", control_url, &headers, Some(body.as_bytes())) {
            Ok((200, b, _)) => return Ok(b),
            Ok((status, _, _)) => {
                last = format!("Browse {object_id}: HTTP {status}");
                std::thread::sleep(Duration::from_millis(400 * (attempt + 1)));
            }
            Err(e) => {
                last = e;
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

// FALLBACK_HOSTS are probed when SSDP can't run. iSH blocks multicast (no iOS
// entitlement) and exposes no routing table (no netlink, no /proc/net/route), so
// real discovery is impossible there — we fall back to the known Sony AP
// addresses. 10.0.0.1 (this firmware) is listed first; it is on the camera AP
// subnet, so the probe is fast. --camera-host overrides for anything else.
// FALLBACK_HOSTS are tried last if we can't derive candidates from our own IP.
// 10.0.0.1 is this firmware's AP gateway; 192.168.122.1 is Sony's documented one.
const FALLBACK_HOSTS: &[&str] = &["10.0.0.1", "192.168.122.1"];

// discover finds the camera's device-description URL. SSDP multicast first
// (works on desktop). On iSH multicast is blocked and there's no routing table,
// so we derive candidate gateway addresses from our own IP (the camera is the
// AP gateway, almost always the ".1" of the subnet) and probe them.
fn discover() -> Option<String> {
    if let Some(loc) = ssdp_discover(Duration::from_secs(5)) {
        return Some(loc);
    }
    eprintln!("SSDP unavailable; deriving candidate camera addresses from local IP...");

    let mut hosts: Vec<String> = Vec::new();
    match local_ip() {
        Some(ip) => {
            eprintln!("local IP (via getsockname): {ip}");
            hosts.extend(candidate_gateways(ip));
        }
        None => eprintln!("could not determine local IP (getsockname gave nothing)"),
    }
    for h in FALLBACK_HOSTS {
        hosts.push((*h).to_string());
    }

    let mut seen = HashSet::new();
    for host in hosts {
        if seen.insert(host.clone()) {
            if let Some(url) = probe_host(&host) {
                return Some(url);
            }
        }
    }
    None
}

// local_ip discovers our own IPv4 via the getsockname trick: connect() a UDP
// socket (no packet is sent) so the OS picks a source address for that route,
// then read it back. Avoids netlink/ifconfig, which iSH lacks.
fn local_ip() -> Option<Ipv4Addr> {
    for dest in ["10.0.0.1:9", "192.168.0.1:9", "172.16.0.1:9", "1.1.1.1:9"] {
        let sock = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(_) => continue,
        };
        if sock.connect(dest).is_ok() {
            if let Ok(local) = sock.local_addr() {
                if let IpAddr::V4(ip) = local.ip() {
                    if !ip.is_unspecified() && !ip.is_loopback() {
                        return Some(ip);
                    }
                }
            }
        }
    }
    None
}

// candidate_gateways guesses the AP gateway (= camera) from our IP: the ".1" at
// the /24, /16 and /8 boundaries, which covers the usual camera AP layouts.
fn candidate_gateways(ip: Ipv4Addr) -> Vec<String> {
    let o = ip.octets();
    vec![
        format!("{}.{}.{}.1", o[0], o[1], o[2]),
        format!("{}.{}.0.1", o[0], o[1]),
        format!("{}.0.0.1", o[0]),
    ]
}

// probe_host fetches http://host:64321/DmsDesc.xml and accepts it only if it
// looks like a Sony camera (so a router/NAS at that address isn't mistaken for one).
fn probe_host(host: &str) -> Option<String> {
    let url = format!("http://{host}:{DEFAULT_DESC_PORT}/DmsDesc.xml");
    eprintln!("probe: GET {url}");
    match http_request("GET", &url, &[("User-Agent", UA)], None) {
        Ok((200, body, _)) if looks_like_sony(&body) => {
            eprintln!("probe: found a Sony camera at {host}");
            Some(url)
        }
        Ok((status, _, _)) => {
            eprintln!("probe: {host} -> HTTP {status} (not a Sony camera)");
            None
        }
        Err(e) => {
            eprintln!("probe: {host} -> {e}");
            None
        }
    }
}

fn looks_like_sony(body: &[u8]) -> bool {
    let s = String::from_utf8_lossy(body).to_lowercase();
    s.contains("sony") || s.contains("imagingdevice")
}

// ssdp_discover sends an M-SEARCH and listens for a MediaServer's LOCATION.
//
// It avoids set_read_timeout (SO_RCVTIMEO), which iSH rejects with EINVAL —
// instead it polls a non-blocking socket. It also reports send/recv errors so
// we can tell on iSH whether multicast actually works (vs. silently failing).
fn ssdp_discover(timeout: Duration) -> Option<String> {
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ssdp: bind failed: {e}");
            return None;
        }
    };
    if let Err(e) = sock.set_nonblocking(true) {
        eprintln!("ssdp: set_nonblocking failed (continuing anyway): {e}");
    }

    let mut sent = 0;
    for st in ["urn:schemas-upnp-org:device:MediaServer:1", "ssdp:all"] {
        let msg = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n\
MAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {st}\r\n\r\n"
        );
        match sock.send_to(msg.as_bytes(), "239.255.255.250:1900") {
            Ok(_) => sent += 1,
            Err(e) => eprintln!("ssdp: send ({st}) failed: {e}"),
        }
    }
    if sent == 0 {
        eprintln!("ssdp: could not send any M-SEARCH (multicast likely unsupported here)");
        return None;
    }
    eprintln!("ssdp: sent {sent} M-SEARCH, listening up to {}s...", timeout.as_secs());

    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 65535];
    while Instant::now() < deadline {
        match sock.recv_from(&mut buf) {
            Ok((n, addr)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                for line in text.split("\r\n") {
                    if let Some((k, v)) = line.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("location") {
                            let loc = v.trim().to_string();
                            eprintln!("ssdp: reply from {addr} -> {loc}");
                            return Some(loc);
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("ssdp: recv error: {e}");
                break;
            }
        }
    }
    eprintln!("ssdp: no replies within timeout");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn split_url_parses_authority_and_path() {
        assert_eq!(
            split_url("http://10.0.0.1:64321/DmsDesc.xml").unwrap(),
            ("10.0.0.1".to_string(), 64321, "/DmsDesc.xml".to_string())
        );
        assert_eq!(
            split_url("http://host").unwrap(),
            ("host".to_string(), 80, "/".to_string())
        );
    }

    #[test]
    fn http_request_round_trips() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            s.write_all(b"HTTP/1.0 200 OK\r\nContent-Type: image/jpeg\r\n\r\nDATA")
                .unwrap();
        });
        let url = format!("http://127.0.0.1:{}/x", addr.port());
        let (status, body, ct) = http_request("GET", &url, &[("User-Agent", "t")], None).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, b"DATA");
        assert_eq!(ct, "image/jpeg");
        server.join().unwrap();
    }
}
