#!/usr/bin/env bash
#
# grab.sh — OFFLINE, self-contained camera diagnostic + fixture capture.
#
# Run this WHILE joined to the Sony camera's Wi-Fi AP. It needs no internet and
# no back-and-forth: it writes EVERYTHING (commands, headers, errors, SSDP
# replies) to camera/testdata/grab.log and saves any fixtures it manages to
# fetch. When done, switch back to your normal Wi-Fi and share grab.log.
#
# On the camera first: Menu → Send to Smartphone → "Select on Smartphone".
# Join SSID DIRECT-xxxx:ILCE-6000 (password = press the camera's trash button).
#
# Usage:  ./scripts/grab.sh                 # default host 192.168.122.1
#         ./scripts/grab.sh 192.168.122.1   # explicit host
#
# Exit code is always 0 — read grab.log for what happened.

HOST="${1:-192.168.122.1}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$ROOT/camera/testdata"
LOG="$OUT_DIR/grab.log"
mkdir -p "$OUT_DIR"

# Mirror all stdout/stderr into the log file.
exec > >(tee "$LOG") 2>&1

hr() { printf '\n========== %s ==========\n' "$1"; }

echo "grab.sh run — host=$HOST"
echo "output dir: $OUT_DIR"

hr "0. Which network am I on?"
# Show the interface that carries the camera subnet + the default route.
ifconfig 2>/dev/null | grep -E 'inet (192\.168\.122\.|10\.|172\.|192\.168\.)' || echo "  (no ifconfig inet lines)"
echo "-- route to $HOST --"
route -n get "$HOST" 2>/dev/null | grep -E 'interface|gateway' || echo "  (no route info)"

hr "1. Reachability (ping)"
ping -c 2 -t 3 "$HOST" 2>&1 || echo "  ping failed/blocked (camera may still answer HTTP)"

hr "2. SSDP discovery (find the REAL description URL)"
# 503 on DmsDescPush.xml often means the path/port differs by firmware. SSDP
# M-SEARCH asks every UPnP device on the AP for its description LOCATION.
if command -v python3 >/dev/null 2>&1; then
python3 - "$HOST" <<'PY'
import socket, sys, time
host = sys.argv[1]
targets = [
    "urn:schemas-upnp-org:device:MediaServer:1",
    "urn:schemas-upnp-org:service:ContentDirectory:1",
    "ssdp:all",
]
tmpl = ("M-SEARCH * HTTP/1.1\r\n"
        "HOST: 239.255.255.250:1900\r\n"
        'MAN: "ssdp:discover"\r\n'
        "MX: 2\r\n"
        "ST: {st}\r\n\r\n")
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 2)
s.settimeout(2)
for st in targets:
    try:
        s.sendto(tmpl.format(st=st).encode(), ("239.255.255.250", 1900))
    except Exception as e:
        print("  send error for", st, "->", e)
seen, locations = set(), []
end = time.time() + 7
print("  listening ~7s for SSDP replies and NOTIFYs...")
while time.time() < end:
    try:
        data, addr = s.recvfrom(65535)
    except socket.timeout:
        continue
    except Exception as e:
        print("  recv error:", e); break
    text = data.decode("utf-8", "replace")
    loc = ""
    for line in text.splitlines():
        if line.lower().startswith("location:"):
            loc = line.split(":", 1)[1].strip()
    key = (addr[0], loc, text[:12])
    if key in seen:
        continue
    seen.add(key)
    print("  ---- from %s:%s" % addr)
    for line in text.strip().splitlines():
        print("    " + line)
    if loc and loc not in locations:
        locations.append(loc)
if locations:
    print("\n  >> Discovered LOCATION URLs:")
    for l in locations:
        print("     " + l)
    with open(sys.argv[0].rsplit("/",1)[0] + "/.ssdp_locations", "w") as f:
        pass
else:
    print("\n  >> No SSDP responses. Camera may not advertise, or multicast is blocked.")
# Hand the first MediaServer-ish location back to the shell via a file.
import os
loc_out = os.path.join(os.path.dirname(os.path.abspath(__file__)) if False else ".", "")
print("LOCATIONS=" + "|".join(locations))
PY
else
  echo "  python3 not found — skipping SSDP. (Install: xcode-select --install)"
  echo "  Fallback: trying a passive listen with nc for 6s..."
  ( nc -u -l -w 6 239.255.255.250 1900 2>/dev/null | head -40 ) || echo "  nc listen unavailable"
fi

# Collect candidate description URLs: anything SSDP printed, plus known guesses.
CANDIDATES=$(grep -Eo 'http://[0-9.]+:[0-9]+/[A-Za-z0-9_./-]+\.xml' "$LOG" 2>/dev/null | sort -u)
CANDIDATES="$CANDIDATES
http://$HOST:64321/DmsDescPush.xml
http://$HOST:64321/DmsRtpd.xml
http://$HOST:60151/DeviceDescription.xml
http://$HOST:8200/rootDesc.xml"

hr "3. Fetch each candidate description (verbose, with retries + UPnP UA)"
DESC_FILE=""
for url in $(printf '%s\n' $CANDIDATES | awk 'NF' | sort -u); do
  echo "-- GET $url"
  for attempt in 1 2 3; do
    code=$(curl -s -o "$OUT_DIR/.try.xml" -w '%{http_code}' --max-time 6 \
                -A "UPnP/1.0 DLNADOC/1.50 Sony" -H 'Accept: */*' "$url" 2>/dev/null)
    size=$(wc -c < "$OUT_DIR/.try.xml" 2>/dev/null | tr -d ' ')
    echo "   attempt $attempt -> HTTP $code, ${size:-0} bytes"
    if [ "$code" = "200" ] && [ "${size:-0}" -gt 80 ]; then
      cp "$OUT_DIR/.try.xml" "$OUT_DIR/DmsDescPush.xml"
      DESC_FILE="$OUT_DIR/DmsDescPush.xml"
      echo "   ✓ saved description -> $DESC_FILE"
      break 2
    fi
    sleep 2
  done
done
rm -f "$OUT_DIR/.try.xml"

if [ -z "$DESC_FILE" ]; then
  hr "RESULT"
  echo "Could not fetch a usable device description."
  echo "Share camera/testdata/grab.log so we can see the SSDP replies + HTTP codes."
  echo "Tips to try, then re-run: re-select 'Select on Smartphone' on the camera;"
  echo "make sure it's NOT 'Ctrl w/ Smartphone' (remote shooting is a different API)."
  exit 0
fi

hr "4. Parse ContentDirectory controlURL from the description"
echo "-- description head --"; head -c 1200 "$DESC_FILE"; echo
CD='//*[local-name()="service"][*[local-name()="serviceType" and contains(.,"ContentDirectory")]]'
CONTROL_URL=$(xmllint --xpath "string($CD/*[local-name()=\"controlURL\"])" "$DESC_FILE" 2>/dev/null)
SERVICE_TYPE=$(xmllint --xpath "string($CD/*[local-name()=\"serviceType\"])" "$DESC_FILE" 2>/dev/null)
BASE=$(printf '%s' "$DESC_FILE" >/dev/null; echo "http://$HOST:64321")
# Resolve relative controlURL against the host that served the description.
SRV_BASE=$(grep -Eo 'http://[0-9.]+:[0-9]+' "$LOG" | grep "$HOST" | head -1)
[ -n "$SRV_BASE" ] && BASE="$SRV_BASE"
case "$CONTROL_URL" in
  http*) : ;;
  /*)    CONTROL_URL="$BASE$CONTROL_URL" ;;
  "")    echo "  !! no ContentDirectory controlURL found"; ;;
  *)     CONTROL_URL="$BASE/$CONTROL_URL" ;;
esac
echo "  serviceType: ${SERVICE_TYPE:-<none>}"
echo "  controlURL : ${CONTROL_URL:-<none>}"

if [ -z "$CONTROL_URL" ] || [ -z "$SERVICE_TYPE" ]; then
  hr "RESULT"
  echo "Got the description but no ContentDirectory service in it."
  echo "Share grab.log + camera/testdata/DmsDescPush.xml."
  exit 0
fi

hr "5. Browse (ObjectID=0, BrowseDirectChildren)"
BODY='<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body><u:Browse xmlns:u="'"$SERVICE_TYPE"'">
    <ObjectID>0</ObjectID><BrowseFlag>BrowseDirectChildren</BrowseFlag>
    <Filter>*</Filter><StartingIndex>0</StartingIndex>
    <RequestedCount>50</RequestedCount><SortCriteria></SortCriteria>
  </u:Browse></s:Body></s:Envelope>'
code=$(curl -s -o "$OUT_DIR/browse_response.xml" -w '%{http_code}' --max-time 15 \
            -A "UPnP/1.0 DLNADOC/1.50 Sony" \
            -H 'Content-Type: text/xml; charset="utf-8"' \
            -H "SOAPACTION: \"$SERVICE_TYPE#Browse\"" \
            --data "$BODY" "$CONTROL_URL")
echo "  Browse -> HTTP $code, $(wc -c < "$OUT_DIR/browse_response.xml" | tr -d ' ') bytes"
echo "  saved -> $OUT_DIR/browse_response.xml"

# Unescape the DIDL-Lite <Result> so the <res> URL shapes are readable.
if command -v python3 >/dev/null 2>&1; then
  xmllint --xpath 'string(//*[local-name()="Result"])' "$OUT_DIR/browse_response.xml" 2>/dev/null \
    | python3 -c 'import sys,html; print(html.unescape(sys.stdin.read()))' \
    > "$OUT_DIR/browse_result.xml" 2>/dev/null && \
    echo "  unescaped DIDL -> $OUT_DIR/browse_result.xml"
fi

hr "RESULT"
echo "Captured fixtures in camera/testdata/:"
ls -la "$OUT_DIR" | grep -vE '\.log$' | sed 's/^/  /'
echo
echo "Now switch back to your normal Wi-Fi and share camera/testdata/grab.log"
echo "(and browse_result.xml if it exists). I'll pin the parser to it."
exit 0
