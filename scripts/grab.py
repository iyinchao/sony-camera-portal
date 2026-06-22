#!/usr/bin/env python3
"""
grab.py — OFFLINE Sony camera DLNA capture (no internet, no back-and-forth).

Run while joined to the camera's Wi-Fi AP. It:
  1. SSDP-discovers the camera (no hard-coded IP — handles 192.168.122.1,
     10.0.0.1, etc.),
  2. fetches the device description and finds the ContentDirectory control URL,
  3. recursively Browses from the root container until it collects real photo
     items (with <res> thumbnail/original URLs), retrying through 503s,
  4. writes fixtures + a full log to camera/testdata/.

Everything goes to camera/testdata/grab.log. When done, switch back to your
normal Wi-Fi and share that log (and browse_result.xml).

Usage:  python3 scripts/grab.py            # auto-discover
        python3 scripts/grab.py 10.0.0.1   # force a host if SSDP is blocked
"""
import os, sys, time, socket, urllib.request, urllib.error
import xml.etree.ElementTree as ET
from urllib.parse import urljoin, urlsplit

OUT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "camera", "testdata"))
os.makedirs(OUT_DIR, exist_ok=True)
_logf = open(os.path.join(OUT_DIR, "grab.log"), "w")
UA = "UPnP/1.0 DLNADOC/1.50 Sony"


def log(*a):
    s = " ".join(str(x) for x in a)
    print(s)
    _logf.write(s + "\n")
    _logf.flush()


def hr(t):
    log("\n========== %s ==========" % t)


def local(tag):
    return tag.split("}")[-1]


def ssdp_discover(timeout=7):
    """Return list of (location_url, server, ip) advertised on the AP."""
    msg = ("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n"
           'MAN: "ssdp:discover"\r\nMX: 2\r\nST: {st}\r\n\r\n')
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 2)
    sock.settimeout(2)
    for st in ("urn:schemas-upnp-org:device:MediaServer:1", "ssdp:all"):
        try:
            sock.sendto(msg.format(st=st).encode(), ("239.255.255.250", 1900))
        except OSError as e:
            log("  ssdp send error:", e)
    found, seen = [], set()
    end = time.time() + timeout
    log("  listening ~%ds for SSDP replies / NOTIFYs..." % timeout)
    while time.time() < end:
        try:
            data, addr = sock.recvfrom(65535)
        except socket.timeout:
            continue
        except OSError:
            break
        text = data.decode("utf-8", "replace")
        loc = server = ""
        for ln in text.splitlines():
            low = ln.lower()
            if low.startswith("location:"):
                loc = ln.split(":", 1)[1].strip()
            elif low.startswith("server:"):
                server = ln.split(":", 1)[1].strip()
        if loc and loc not in seen:
            seen.add(loc)
            found.append((loc, server, addr[0]))
            log("  found: %s   (%s)" % (loc, server))
    return found


def http_get(url, timeout=8):
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "*/*"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, r.read()


def find_contentdir(desc_bytes, desc_url):
    """Return (serviceType, absolute controlURL) for ContentDirectory, or (None,None)."""
    root = ET.fromstring(desc_bytes)
    for svc in root.iter():
        if local(svc.tag) != "service":
            continue
        st = cu = None
        for ch in svc:
            if local(ch.tag) == "serviceType":
                st = (ch.text or "").strip()
            elif local(ch.tag) == "controlURL":
                cu = (ch.text or "").strip()
        if st and "ContentDirectory" in st and cu:
            return st, urljoin(desc_url, cu)
    return None, None


def browse(control_url, service_type, object_id="0", start=0, count=50, timeout=15):
    body = ('<?xml version="1.0" encoding="utf-8"?>'
            '<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" '
            's:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body>'
            '<u:Browse xmlns:u="%s"><ObjectID>%s</ObjectID>'
            '<BrowseFlag>BrowseDirectChildren</BrowseFlag><Filter>*</Filter>'
            '<StartingIndex>%d</StartingIndex><RequestedCount>%d</RequestedCount>'
            '<SortCriteria></SortCriteria></u:Browse></s:Body></s:Envelope>'
            % (service_type, object_id, start, count))
    req = urllib.request.Request(
        control_url, data=body.encode(),
        headers={"User-Agent": UA,
                 "Content-Type": 'text/xml; charset="utf-8"',
                 "SOAPACTION": '"%s#Browse"' % service_type})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, r.read()


def extract_result(soap_bytes):
    """Pull the (already-unescaped) DIDL-Lite string out of a Browse response."""
    root = ET.fromstring(soap_bytes)
    for el in root.iter():
        if local(el.tag) == "Result":
            return el.text or ""
    return ""


def parse_didl(didl_str):
    if not didl_str.strip():
        return [], []
    root = ET.fromstring(didl_str)
    containers, items = [], []
    for el in root:
        tag = local(el.tag)
        oid = el.get("id")
        title = cls = None
        ress = []
        for ch in el:
            lt = local(ch.tag)
            if lt == "title":
                title = ch.text
            elif lt == "class":
                cls = ch.text or ""
            elif lt == "res":
                ress.append({"url": (ch.text or "").strip(),
                             "protocolInfo": ch.get("protocolInfo"),
                             "resolution": ch.get("resolution"),
                             "size": ch.get("size")})
        rec = {"id": oid, "title": title, "class": cls, "res": ress}
        (containers if tag == "container" else items).append(rec)
    return containers, items


def main():
    forced_host = sys.argv[1] if len(sys.argv) > 1 else None
    log("grab.py — output dir:", OUT_DIR)

    hr("0. Network")
    try:
        import subprocess
        out = subprocess.run(["ifconfig"], capture_output=True, text=True).stdout
        for ln in out.splitlines():
            if "inet " in ln:
                log("  " + ln.strip())
    except Exception as e:
        log("  (ifconfig unavailable:", e, ")")

    hr("1. SSDP discovery")
    desc_urls = []
    if forced_host:
        log("  forced host given; will also try SSDP")
    for loc, server, ip in ssdp_discover():
        if loc not in desc_urls:
            desc_urls.append(loc)
    if forced_host:
        for p in (64321, 60151, 8200):
            for name in ("DmsDesc.xml", "DmsDescPush.xml", "rootDesc.xml"):
                desc_urls.append("http://%s:%d/%s" % (forced_host, p, name))
    if not desc_urls:
        log("  No SSDP responses and no forced host. Re-run as:")
        log("    python3 scripts/grab.py <camera-ip>")
        return

    hr("2. Fetch device description")
    desc_url = desc_bytes = None
    for url in desc_urls:
        log("-- GET", url)
        for attempt in (1, 2, 3):
            try:
                status, data = http_get(url)
                log("   attempt %d -> HTTP %d, %d bytes" % (attempt, status, len(data)))
                if status == 200 and len(data) > 80:
                    desc_url, desc_bytes = url, data
                    break
            except urllib.error.HTTPError as e:
                log("   attempt %d -> HTTP %d" % (attempt, e.code))
            except Exception as e:
                log("   attempt %d -> error: %s" % (attempt, e))
            time.sleep(1.5)
        if desc_bytes:
            break
    if not desc_bytes:
        hr("RESULT")
        log("Could not fetch a device description. Share grab.log.")
        return
    with open(os.path.join(OUT_DIR, "DmsDesc.xml"), "wb") as f:
        f.write(desc_bytes)
    log("  saved DmsDesc.xml from", desc_url)

    hr("3. ContentDirectory service")
    service_type, control_url = find_contentdir(desc_bytes, desc_url)
    log("  serviceType:", service_type)
    log("  controlURL :", control_url)
    if not control_url:
        hr("RESULT")
        log("No ContentDirectory in the description. Share grab.log + DmsDesc.xml.")
        return

    hr("4. Recursive Browse (root -> containers -> items)")
    queue, visited = ["0"], set()
    all_items, raw_with_items = [], None
    root_raw = None
    while queue and len(all_items) < 12 and len(visited) < 60:
        oid = queue.pop(0)
        if oid in visited:
            continue
        visited.add(oid)
        data = None
        for attempt in (1, 2, 3):
            try:
                status, data = browse(control_url, service_type, oid)
                break
            except urllib.error.HTTPError as e:
                log("  browse id=%s attempt %d -> HTTP %d" % (oid, attempt, e.code))
                time.sleep(1.5)
                data = None
            except Exception as e:
                log("  browse id=%s error: %s" % (oid, e))
                data = None
                break
        if not data:
            continue
        if oid == "0":
            root_raw = data
        try:
            result = extract_result(data)
            conts, items = parse_didl(result)
        except Exception as e:
            log("  parse error on id=%s: %s" % (oid, e))
            continue
        log("  browse id=%s -> %d containers, %d items" % (oid, len(conts), len(items)))
        if items:
            if raw_with_items is None:
                raw_with_items = data
            all_items.extend(items)
        for c in conts:
            if c["id"] and c["id"] not in visited:
                queue.append(c["id"])

    # Save the most useful raw response as the parser fixture.
    fixture = raw_with_items if raw_with_items is not None else root_raw
    if fixture is not None:
        with open(os.path.join(OUT_DIR, "browse_response.xml"), "wb") as f:
            f.write(fixture)
        log("  saved browse_response.xml (%d bytes)" % len(fixture))
        try:
            with open(os.path.join(OUT_DIR, "browse_result.xml"), "w") as f:
                f.write(extract_result(fixture))
            log("  saved browse_result.xml (unescaped DIDL)")
        except Exception as e:
            log("  could not write browse_result.xml:", e)

    hr("5. Sample of collected photo items")
    if not all_items:
        log("  No image items found. Root browse may need a different container,")
        log("  or the camera is still 'connecting'. Share grab.log + browse_response.xml.")
    for it in all_items[:6]:
        log("  - id=%s  title=%s  class=%s" % (it["id"], it["title"], it["class"]))
        for r in it["res"]:
            log("      res: proto=%s res=%s size=%s" % (r["protocolInfo"], r["resolution"], r["size"]))
            log("           url=%s" % r["url"])

    hr("RESULT")
    log("Items collected: %d" % len(all_items))
    log("Fixtures in camera/testdata/: DmsDesc.xml, browse_response.xml, browse_result.xml, grab.log")
    log("Switch back to normal Wi-Fi and share grab.log (+ browse_result.xml).")


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        log("FATAL:", e)
    finally:
        _logf.close()
