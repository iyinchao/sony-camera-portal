package camera

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func readFixture(t *testing.T, name string) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", name))
	if err != nil {
		t.Fatalf("read fixture %s: %v", name, err)
	}
	return data
}

func TestParseDeviceDescription(t *testing.T) {
	data := readFixture(t, "DmsDesc.xml")
	const descURL = "http://10.0.0.1:64321/DmsDesc.xml"

	controlURL, serviceType, err := parseDeviceDescription(data, descURL)
	if err != nil {
		t.Fatalf("parseDeviceDescription: %v", err)
	}
	if want := "http://10.0.0.1:64321/upnp/control/ContentDirectory"; controlURL != want {
		t.Errorf("controlURL = %q, want %q", controlURL, want)
	}
	if want := "urn:schemas-upnp-org:service:ContentDirectory:1"; serviceType != want {
		t.Errorf("serviceType = %q, want %q", serviceType, want)
	}
}

func TestParseDeviceDescriptionErrors(t *testing.T) {
	tests := []struct {
		name string
		data string
	}{
		{"not xml", "this is not xml"},
		{"no contentdirectory", `<root><device><serviceList></serviceList></device></root>`},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if _, _, err := parseDeviceDescription([]byte(tc.data), "http://10.0.0.1/DmsDesc.xml"); err == nil {
				t.Fatal("expected an error, got nil")
			}
		})
	}
}

func TestParseBrowseResponse(t *testing.T) {
	data := readFixture(t, "browse_response.xml")

	items, containers, numReturned, total, err := parseBrowseResponse(data)
	if err != nil {
		t.Fatalf("parseBrowseResponse: %v", err)
	}
	if len(containers) != 0 {
		t.Errorf("containers = %d, want 0 (fixture is a leaf container)", len(containers))
	}
	if numReturned != 4 || total != 4 {
		t.Errorf("paging = (%d,%d), want (4,4)", numReturned, total)
	}
	if len(items) != 4 {
		t.Fatalf("items = %d, want 4", len(items))
	}

	got := items[0]
	want := Photo{
		ID:       "04_02_0326702136_000001_000001_000000",
		Name:     "DSC07000.JPG",
		Date:     "2014-01-01T00:00:10",
		ThumbURL: "http://10.0.0.1:60151/TN_DSC07000.JPG",
		FullURL:  "http://10.0.0.1:60151/ORG_DSC07000.JPG",
	}
	if got.ID != want.ID {
		t.Errorf("ID = %q, want %q", got.ID, want.ID)
	}
	if got.Name != want.Name {
		t.Errorf("Name = %q, want %q", got.Name, want.Name)
	}
	if got.Date != want.Date {
		t.Errorf("Date = %q, want %q", got.Date, want.Date)
	}
	if !strings.HasPrefix(got.ThumbURL, want.ThumbURL) {
		t.Errorf("ThumbURL = %q, want prefix %q (JPEG_TN)", got.ThumbURL, want.ThumbURL)
	}
	if !strings.HasPrefix(got.FullURL, want.FullURL) {
		t.Errorf("FullURL = %q, want prefix %q (original)", got.FullURL, want.FullURL)
	}
}

func TestParseBrowseResponseJPEGOnly(t *testing.T) {
	items, _, _, _, err := parseBrowseResponse(readFixture(t, "browse_response.xml"))
	if err != nil {
		t.Fatalf("parseBrowseResponse: %v", err)
	}
	for _, p := range items {
		if strings.Contains(strings.ToUpper(p.FullURL), ".ARW") {
			t.Errorf("%s: full URL advertises RAW: %s", p.Name, p.FullURL)
		}
	}
}

func TestDLNAPN(t *testing.T) {
	tests := []struct {
		protocolInfo string
		want         string
	}{
		{"http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_TN;DLNA.ORG_CI=1", "JPEG_TN"},
		{"http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_LRG;DLNA.ORG_CI=1", "JPEG_LRG"},
		{"http-get:*:image/jpeg:*", ""},
		{"", ""},
	}
	for _, tc := range tests {
		if got := dlnaPN(tc.protocolInfo); got != tc.want {
			t.Errorf("dlnaPN(%q) = %q, want %q", tc.protocolInfo, got, tc.want)
		}
	}
}

func TestSelectURLs(t *testing.T) {
	res := []didlRes{
		{ProtocolInfo: "http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_LRG", URL: "http://h/LRG.JPG"},
		{ProtocolInfo: "http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_SM", URL: "http://h/SM.JPG"},
		{ProtocolInfo: "http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_TN", URL: "http://h/TN.JPG"},
		{ProtocolInfo: "http-get:*:image/jpeg:*", Resolution: "6000x4000", URL: "http://h/ORG.JPG"},
	}
	thumb, full := selectURLs(res)
	if thumb != "http://h/TN.JPG" {
		t.Errorf("thumb = %q, want the JPEG_TN url", thumb)
	}
	if full != "http://h/ORG.JPG" {
		t.Errorf("full = %q, want the PN-less original url", full)
	}

	// Fallbacks when TN / original are missing.
	thumb, full = selectURLs([]didlRes{
		{ProtocolInfo: "http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_LRG", URL: "http://h/LRG.JPG"},
	})
	if thumb != "http://h/LRG.JPG" || full != "http://h/LRG.JPG" {
		t.Errorf("fallback = (%q,%q), want both LRG", thumb, full)
	}
}

func TestSSDPLocation(t *testing.T) {
	resp := "HTTP/1.1 200 OK\r\n" +
		"CACHE-CONTROL: max-age=1800\r\n" +
		"LOCATION: http://10.0.0.1:64321/DmsDesc.xml\r\n" +
		"SERVER: UPnP/1.0 SonyImagingDevice/1.0\r\n\r\n"
	if got := ssdpLocation(resp); got != "http://10.0.0.1:64321/DmsDesc.xml" {
		t.Errorf("ssdpLocation = %q", got)
	}
	if got := ssdpLocation("HTTP/1.1 200 OK\r\nSERVER: x\r\n\r\n"); got != "" {
		t.Errorf("ssdpLocation without LOCATION = %q, want empty", got)
	}
}
