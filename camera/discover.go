package camera

import (
	"context"
	"encoding/xml"
	"errors"
	"fmt"
	"net"
	"net/url"
	"strings"
	"time"
)

// deviceDesc is the subset of the UPnP device description we care about.
// encoding/xml matches by local element name, so namespace prefixes
// (dc:, upnp:, av:) are handled transparently.
type deviceDesc struct {
	Device struct {
		FriendlyName string `xml:"friendlyName"`
		PhotoRoot    string `xml:"photoRoot"` // Sony av:photoRoot shortcut
		ServiceList  struct {
			Services []struct {
				ServiceType string `xml:"serviceType"`
				ControlURL  string `xml:"controlURL"`
			} `xml:"service"`
		} `xml:"serviceList"`
	} `xml:"device"`
}

// parseDeviceDescription extracts the ContentDirectory control URL (resolved to
// an absolute URL against descURL) and its service type.
func parseDeviceDescription(data []byte, descURL string) (controlURL, serviceType string, err error) {
	var d deviceDesc
	if err := xml.Unmarshal(data, &d); err != nil {
		return "", "", fmt.Errorf("parse device description: %w", err)
	}
	base, err := url.Parse(descURL)
	if err != nil {
		return "", "", fmt.Errorf("bad description URL %q: %w", descURL, err)
	}
	for _, s := range d.Device.ServiceList.Services {
		if !strings.Contains(s.ServiceType, "ContentDirectory") {
			continue
		}
		ref, err := url.Parse(strings.TrimSpace(s.ControlURL))
		if err != nil {
			return "", "", fmt.Errorf("bad controlURL %q: %w", s.ControlURL, err)
		}
		return base.ResolveReference(ref).String(), strings.TrimSpace(s.ServiceType), nil
	}
	return "", "", fmt.Errorf("no ContentDirectory service in device description")
}

// ensureService makes sure controlURL/serviceType are known, discovering the
// device via SSDP (or the configured host/DescURL) on first use.
func (c *Client) ensureService(ctx context.Context) error {
	if c.controlURL != "" {
		return nil
	}
	descURL := c.DescURL
	if descURL == "" && c.Host != "" {
		descURL = fmt.Sprintf("http://%s:64321/DmsDesc.xml", c.Host)
	}
	if descURL == "" {
		found, err := Discover(ctx, 5*time.Second)
		if err != nil {
			return err
		}
		descURL = found
	}
	// Any failure to fetch or parse the device description means we're not
	// actually talking to a camera, so surface the friendly not-connected hint.
	data, err := c.get(ctx, descURL)
	if err != nil {
		if errors.Is(err, ErrNotConnected) {
			return err
		}
		return fmt.Errorf("%w: %v", ErrNotConnected, err)
	}
	cu, st, err := parseDeviceDescription(data, descURL)
	if err != nil {
		return fmt.Errorf("%w: %v", ErrNotConnected, err)
	}
	c.controlURL, c.serviceType = cu, st
	return nil
}

// ssdpAddr is the standard UPnP multicast endpoint.
const ssdpAddr = "239.255.255.250:1900"

// Discover performs an SSDP M-SEARCH and returns the first MediaServer device
// description LOCATION it sees. Address/port are not assumed, which is essential
// because the camera's AP IP varies by firmware.
func Discover(ctx context.Context, timeout time.Duration) (descURL string, err error) {
	raddr, err := net.ResolveUDPAddr("udp4", ssdpAddr)
	if err != nil {
		return "", err
	}
	conn, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4zero, Port: 0})
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrNotConnected, err)
	}
	defer conn.Close()

	deadline := time.Now().Add(timeout)
	if d, ok := ctx.Deadline(); ok && d.Before(deadline) {
		deadline = d
	}
	_ = conn.SetDeadline(deadline)

	for _, st := range []string{
		"urn:schemas-upnp-org:device:MediaServer:1",
		"ssdp:all",
	} {
		msg := "M-SEARCH * HTTP/1.1\r\n" +
			"HOST: " + ssdpAddr + "\r\n" +
			"MAN: \"ssdp:discover\"\r\n" +
			"MX: 2\r\n" +
			"ST: " + st + "\r\n\r\n"
		if _, err := conn.WriteToUDP([]byte(msg), raddr); err != nil {
			return "", fmt.Errorf("%w: %v", ErrNotConnected, err)
		}
	}

	buf := make([]byte, 65535)
	for {
		select {
		case <-ctx.Done():
			return "", ctx.Err()
		default:
		}
		n, _, err := conn.ReadFromUDP(buf)
		if err != nil {
			return "", fmt.Errorf("no camera found via SSDP: %w", err)
		}
		if loc := ssdpLocation(string(buf[:n])); loc != "" {
			return loc, nil
		}
	}
}

// ssdpLocation returns the LOCATION header value from an SSDP response, or "".
func ssdpLocation(resp string) string {
	for _, line := range strings.Split(resp, "\r\n") {
		if k, v, ok := strings.Cut(line, ":"); ok && strings.EqualFold(strings.TrimSpace(k), "location") {
			return strings.TrimSpace(v)
		}
	}
	return ""
}
