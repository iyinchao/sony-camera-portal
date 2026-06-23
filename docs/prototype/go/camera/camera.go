// Package camera talks to a Sony PlayMemories / Imaging Edge camera (e.g. the
// a6000) over its Wi-Fi access point: it discovers the UPnP ContentDirectory,
// enumerates photos via Browse, and fetches image bytes.
//
// Protocol notes (confirmed against a real ILCE-6000):
//   - SSDP advertises the device description, e.g.
//     http://10.0.0.1:64321/DmsDesc.xml. Host/port/filename vary by firmware;
//     the SPEC's 192.168.122.1 / DmsDescPush.xml is only a documented fallback,
//     so we discover via SSDP rather than assume.
//   - The ContentDirectory controlURL is relative in the description
//     (/upnp/control/ContentDirectory) and resolves against the description URL.
//   - Photos live a few container levels below the root: 0 -> PhotoRoot ->
//     a grouping container -> date containers -> image items. A recursive
//     BrowseDirectChildren from "0" reaches them.
//   - Each image item exposes four <res>, distinguished by DLNA.ORG_PN in
//     protocolInfo: JPEG_TN (thumbnail), JPEG_SM, JPEG_LRG, and a PN-less entry
//     that is the full-resolution original. All are image/jpeg, served from a
//     separate media port (e.g. :60151).
package camera

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"time"
)

// DefaultHost is the camera's gateway IP in AP mode per Sony's documentation.
// Real firmware may differ (e.g. 10.0.0.1); discovery overrides this.
const DefaultHost = "192.168.122.1"

// userAgent placates Sony's DLNA stack, which can 503 unrecognised clients.
const userAgent = "UPnP/1.0 DLNADOC/1.50 Sony"

// ErrNotConnected indicates the client could not reach the camera at all,
// almost always because the host is not joined to the camera's Wi-Fi AP.
var ErrNotConnected = errors.New("not connected to the camera (join its Wi-Fi access point)")

// Photo is one image enumerated from the camera. ThumbURL and FullURL are the
// camera's own media URLs; the server proxies them so the browser never talks
// to the camera directly.
type Photo struct {
	ID       string
	Name     string
	Date     string // raw dc:date (ISO 8601), as reported by the camera
	ThumbURL string // DLNA JPEG_TN resource
	FullURL  string // full-resolution original JPEG resource
}

// Client enumerates and fetches photos from one camera.
//
// The zero value is usable: List discovers the device via SSDP when DescURL is
// empty. Set Host to skip discovery against a known address, or DescURL to pin
// the exact device-description URL.
type Client struct {
	HTTP    *http.Client // optional; a sensible default is used when nil
	Host    string       // optional fixed host; defaults to DefaultHost
	DescURL string       // optional exact device-description URL

	controlURL  string // resolved ContentDirectory control URL
	serviceType string // ContentDirectory service type
}

var defaultHTTP = &http.Client{Timeout: 15 * time.Second}

func (c *Client) httpClient() *http.Client {
	if c.HTTP != nil {
		return c.HTTP
	}
	return defaultHTTP
}

// get fetches a URL and returns its body, mapping dial failures to
// ErrNotConnected so callers can show a friendly "join the camera Wi-Fi" hint.
func (c *Client) get(ctx context.Context, url string) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", userAgent)
	resp, err := c.httpClient().Do(req)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrNotConnected, err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("GET %s: %s", url, resp.Status)
	}
	return io.ReadAll(resp.Body)
}

// Open streams an arbitrary camera media URL (a photo's ThumbURL or FullURL) for
// the server to proxy to the browser. The caller must Close the returned reader.
func (c *Client) Open(ctx context.Context, url string) (body io.ReadCloser, contentType string, err error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, "", err
	}
	req.Header.Set("User-Agent", userAgent)
	resp, err := c.httpClient().Do(req)
	if err != nil {
		return nil, "", fmt.Errorf("%w: %v", ErrNotConnected, err)
	}
	if resp.StatusCode != http.StatusOK {
		resp.Body.Close()
		return nil, "", fmt.Errorf("GET %s: %s", url, resp.Status)
	}
	ct := resp.Header.Get("Content-Type")
	if ct == "" {
		ct = "image/jpeg"
	}
	return resp.Body, ct, nil
}
