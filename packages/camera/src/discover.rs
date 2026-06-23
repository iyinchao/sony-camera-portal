//! Camera discovery.
//!
//! SSDP multicast works on desktop but is **blocked on iOS/iSH** (no multicast
//! entitlement; send fails with EHOSTUNREACH). iSH also exposes no routing table
//! (no netlink / `/proc/net/route`), so we derive candidate gateway addresses
//! from our own IP (the camera is the AP gateway, almost always the subnet's
//! ".1") and probe them.

use crate::http::{http_request, UA};
use crate::DEFAULT_DESC_PORT;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

/// Tried last when we can't derive candidates from our own IP. 10.0.0.1 is this
/// firmware's AP gateway; 192.168.122.1 is Sony's documented one.
const FALLBACK_HOSTS: &[&str] = &["10.0.0.1", "192.168.122.1"];

/// Find a Sony camera's device-description URL, or None.
pub(crate) fn discover() -> Option<String> {
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

/// Discover our own IPv4 via the getsockname trick: `connect()` a UDP socket (no
/// packet is sent) so the OS picks a source address, then read it back. Avoids
/// netlink/ifconfig, which iSH lacks.
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

/// Guess AP gateways (= camera) from our IP: the ".1" at /24, /16 and /8.
fn candidate_gateways(ip: Ipv4Addr) -> Vec<String> {
    let o = ip.octets();
    vec![
        format!("{}.{}.{}.1", o[0], o[1], o[2]),
        format!("{}.{}.0.1", o[0], o[1]),
        format!("{}.0.0.1", o[0]),
    ]
}

/// Fetch a host's DmsDesc.xml and accept it only if it looks like a Sony camera,
/// so a router/NAS at that address isn't mistaken for one.
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

/// Send an SSDP M-SEARCH and return the first MediaServer LOCATION.
///
/// Avoids `set_read_timeout` (SO_RCVTIMEO), which iSH rejects with EINVAL — it
/// polls a non-blocking socket instead, and reports send/recv errors so we can
/// tell whether multicast works at all.
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
    eprintln!(
        "ssdp: sent {sent} M-SEARCH, listening up to {}s...",
        timeout.as_secs()
    );

    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 65535];
    while Instant::now() < deadline {
        match sock.recv_from(&mut buf) {
            Ok((n, addr)) => {
                // Prefer a Sony camera. Other DLNA servers (Emby/Jellyfin/NAS)
                // also answer, so we ignore non-Sony replies rather than grab
                // the wrong device.
                let text = String::from_utf8_lossy(&buf[..n]);
                let (mut loc, mut server) = ("", "");
                for line in text.split("\r\n") {
                    if let Some((k, v)) = line.split_once(':') {
                        match k.trim().to_ascii_lowercase().as_str() {
                            "location" => loc = v.trim(),
                            "server" => server = v.trim(),
                            _ => {}
                        }
                    }
                }
                if !loc.is_empty() {
                    if looks_like_sony(server.as_bytes()) {
                        eprintln!("ssdp: Sony reply from {addr} -> {loc}");
                        return Some(loc.to_string());
                    }
                    eprintln!("ssdp: ignoring non-Sony reply from {addr} ({server})");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_gateways_covers_boundaries() {
        assert_eq!(
            candidate_gateways("10.0.1.5".parse().unwrap()),
            vec!["10.0.1.1", "10.0.0.1", "10.0.0.1"]
        );
    }

    #[test]
    fn sony_detection() {
        assert!(looks_like_sony(b"<modelName>SonyImagingDevice</modelName>"));
        assert!(!looks_like_sony(b"<modelName>Emby</modelName>"));
    }
}
