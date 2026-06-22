#!/usr/bin/env bash
#
# capture-fixtures.sh — grab real DmsDescPush.xml + a Browse response from a
# Sony PlayMemories camera (e.g. a6000) so we can unit-test the parser offline.
#
# Prereqs:
#   1. On the camera: Menu → Send to Smartphone → "Select on Smartphone".
#   2. Join the camera's Wi-Fi AP (SSID DIRECT-xxxx:ILCE-6000; password is shown
#      via the camera's delete/trash button).
#   3. xmllint + curl on PATH (both ship with macOS).
#
# Usage:
#   scripts/capture-fixtures.sh                 # defaults to 192.168.122.1:64321
#   scripts/capture-fixtures.sh 192.168.122.1   # custom host
#   DESC_URL=http://192.168.122.1:64321/DmsDescPush.xml scripts/capture-fixtures.sh
#
# Output (into camera/testdata/):
#   DmsDescPush.xml         raw device description
#   browse_response.xml     raw Browse SOAP response (Result still DIDL-escaped)
#   browse_result.xml       the unescaped DIDL-Lite (for eyeballing <res> URLs)

set -euo pipefail

HOST="${1:-192.168.122.1}"
DESC_PORT="${DESC_PORT:-64321}"
DESC_URL="${DESC_URL:-http://${HOST}:${DESC_PORT}/DmsDescPush.xml}"
OUT_DIR="$(cd "$(dirname "$0")/.." && pwd)/camera/testdata"
REQUESTED_COUNT="${REQUESTED_COUNT:-50}"

mkdir -p "$OUT_DIR"

echo "==> 1. Fetching device description: $DESC_URL"
curl -fsS --max-time 10 "$DESC_URL" -o "$OUT_DIR/DmsDescPush.xml"
echo "    saved $OUT_DIR/DmsDescPush.xml ($(wc -c <"$OUT_DIR/DmsDescPush.xml") bytes)"

# Extract the ContentDirectory controlURL + serviceType. local-name() sidesteps
# the default UPnP namespace so the XPath works regardless of prefixes.
CD_XPATH="//*[local-name()='service'][*[local-name()='serviceType' and contains(.,'ContentDirectory')]]"
CONTROL_URL="$(xmllint --xpath "string(${CD_XPATH}/*[local-name()='controlURL'])" "$OUT_DIR/DmsDescPush.xml")"
SERVICE_TYPE="$(xmllint --xpath "string(${CD_XPATH}/*[local-name()='serviceType'])" "$OUT_DIR/DmsDescPush.xml")"

if [[ -z "$CONTROL_URL" ]]; then
  echo "!! Could not find a ContentDirectory controlURL in DmsDescPush.xml." >&2
  echo "   Inspect $OUT_DIR/DmsDescPush.xml manually." >&2
  exit 1
fi

# Resolve relative controlURL against the description's scheme://host:port.
if [[ "$CONTROL_URL" != http* ]]; then
  BASE="$(printf '%s' "$DESC_URL" | sed -E 's#(https?://[^/]+).*#\1#')"
  CONTROL_URL="${BASE}${CONTROL_URL}"
fi
echo "==> 2. ContentDirectory"
echo "    serviceType: $SERVICE_TYPE"
echo "    controlURL : $CONTROL_URL"

echo "==> 3. Issuing Browse (ObjectID=0, BrowseDirectChildren, count=$REQUESTED_COUNT)"
SOAP_BODY=$(cat <<XML
<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Browse xmlns:u="${SERVICE_TYPE}">
      <ObjectID>0</ObjectID>
      <BrowseFlag>BrowseDirectChildren</BrowseFlag>
      <Filter>*</Filter>
      <StartingIndex>0</StartingIndex>
      <RequestedCount>${REQUESTED_COUNT}</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>
XML
)

curl -fsS --max-time 15 "$CONTROL_URL" \
  -H 'Content-Type: text/xml; charset="utf-8"' \
  -H "SOAPACTION: \"${SERVICE_TYPE}#Browse\"" \
  --data "$SOAP_BODY" \
  -o "$OUT_DIR/browse_response.xml"
echo "    saved $OUT_DIR/browse_response.xml ($(wc -c <"$OUT_DIR/browse_response.xml") bytes)"

# The <Result> is HTML-escaped DIDL-Lite; pull it out and unescape for reading.
echo "==> 4. Extracting + unescaping DIDL-Lite Result"
xmllint --xpath "string(//*[local-name()='Result'])" "$OUT_DIR/browse_response.xml" \
  | python3 -c 'import sys,html; print(html.unescape(sys.stdin.read()))' \
  > "$OUT_DIR/browse_result.xml" 2>/dev/null \
  || echo "    (python3 not found — open browse_response.xml and unescape <Result> by hand)"

echo ""
echo "Done. Fixtures in $OUT_DIR:"
ls -la "$OUT_DIR"
echo ""
echo "Next: eyeball browse_result.xml for the <res> URL shapes (thumbnail vs"
echo "original) and pin the parser in camera/ to what you actually see."
