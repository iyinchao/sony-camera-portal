package camera

import (
	"context"
	"encoding/xml"
	"fmt"
	"io"
	"net/http"
	"sort"
	"strings"
	"sync"
	"time"
)

// browseEnvelope is the SOAP body for a ContentDirectory BrowseDirectChildren.
// Args: service type, ObjectID, StartingIndex, RequestedCount.
const browseEnvelope = `<?xml version="1.0" encoding="utf-8"?>` +
	`<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">` +
	`<s:Body><u:Browse xmlns:u="%s">` +
	`<ObjectID>%s</ObjectID><BrowseFlag>BrowseDirectChildren</BrowseFlag>` +
	`<Filter>*</Filter><StartingIndex>%d</StartingIndex>` +
	`<RequestedCount>%d</RequestedCount><SortCriteria></SortCriteria>` +
	`</u:Browse></s:Body></s:Envelope>`

const browsePageSize = 50

// browseResponse is the SOAP envelope wrapping a Browse result. The Result field
// holds DIDL-Lite XML as (auto-unescaped) character data.
type browseResponse struct {
	Result         string `xml:"Body>BrowseResponse>Result"`
	NumberReturned int    `xml:"Body>BrowseResponse>NumberReturned"`
	TotalMatches   int    `xml:"Body>BrowseResponse>TotalMatches"`
}

type didlLite struct {
	Containers []didlContainer `xml:"container"`
	Items      []didlItem      `xml:"item"`
}

type didlContainer struct {
	ID string `xml:"id,attr"`
}

type didlItem struct {
	ID    string    `xml:"id,attr"`
	Title string    `xml:"title"` // dc:title
	Class string    `xml:"class"` // upnp:class
	Date  string    `xml:"date"`  // dc:date
	Res   []didlRes `xml:"res"`
}

type didlRes struct {
	ProtocolInfo string `xml:"protocolInfo,attr"`
	Resolution   string `xml:"resolution,attr"`
	Size         string `xml:"size,attr"`
	URL          string `xml:",chardata"`
}

// parseBrowseResponse decodes a Browse SOAP response into typed photos plus the
// IDs of any child containers, and returns the paging counters.
func parseBrowseResponse(data []byte) (items []Photo, containers []string, numberReturned, totalMatches int, err error) {
	var br browseResponse
	if err := xml.Unmarshal(data, &br); err != nil {
		return nil, nil, 0, 0, fmt.Errorf("parse Browse response: %w", err)
	}
	var didl didlLite
	if strings.TrimSpace(br.Result) != "" {
		if err := xml.Unmarshal([]byte(br.Result), &didl); err != nil {
			return nil, nil, 0, 0, fmt.Errorf("parse DIDL-Lite: %w", err)
		}
	}
	for _, ct := range didl.Containers {
		if ct.ID != "" {
			containers = append(containers, ct.ID)
		}
	}
	for _, it := range didl.Items {
		thumb, full := selectURLs(it.Res)
		items = append(items, Photo{
			ID:       it.ID,
			Name:     strings.TrimSpace(it.Title),
			Date:     strings.TrimSpace(it.Date),
			ThumbURL: thumb,
			FullURL:  full,
		})
	}
	return items, containers, br.NumberReturned, br.TotalMatches, nil
}

// dlnaPN extracts the DLNA.ORG_PN profile (e.g. "JPEG_TN") from a protocolInfo
// string, or "" when absent (the original full-resolution resource has none).
func dlnaPN(protocolInfo string) string {
	const key = "DLNA.ORG_PN="
	i := strings.Index(protocolInfo, key)
	if i < 0 {
		return ""
	}
	v := protocolInfo[i+len(key):]
	if j := strings.IndexAny(v, ";:"); j >= 0 {
		v = v[:j]
	}
	return v
}

// selectURLs picks the thumbnail and full-resolution original from an item's
// <res> set by DLNA profile rather than position, which is not guaranteed:
//   - thumb: JPEG_TN, falling back to JPEG_SM, then JPEG_LRG;
//   - full:  the PN-less original, falling back to JPEG_LRG.
func selectURLs(res []didlRes) (thumb, full string) {
	byPN := map[string]string{}
	var original string
	for _, r := range res {
		u := strings.TrimSpace(r.URL)
		if u == "" {
			continue
		}
		pn := dlnaPN(r.ProtocolInfo)
		if pn == "" {
			original = u
		} else if _, ok := byPN[pn]; !ok {
			byPN[pn] = u
		}
	}
	thumb = firstNonEmpty(byPN["JPEG_TN"], byPN["JPEG_SM"], byPN["JPEG_LRG"], original)
	full = firstNonEmpty(original, byPN["JPEG_LRG"], byPN["JPEG_SM"], byPN["JPEG_TN"])
	return thumb, full
}

func firstNonEmpty(vals ...string) string {
	for _, v := range vals {
		if v != "" {
			return v
		}
	}
	return ""
}

// browse issues one Browse request, retrying briefly through Sony's transient
// 503s.
func (c *Client) browse(ctx context.Context, objectID string, start, count int) ([]byte, error) {
	body := fmt.Sprintf(browseEnvelope, c.serviceType, xmlEscape(objectID), start, count)
	var lastErr error
	for attempt := 0; attempt < 3; attempt++ {
		req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.controlURL, strings.NewReader(body))
		if err != nil {
			return nil, err
		}
		req.Header.Set("Content-Type", `text/xml; charset="utf-8"`)
		req.Header.Set("SOAPACTION", `"`+c.serviceType+`#Browse"`)
		req.Header.Set("User-Agent", userAgent)

		resp, err := c.httpClient().Do(req)
		if err != nil {
			return nil, fmt.Errorf("%w: %v", ErrNotConnected, err)
		}
		if resp.StatusCode == http.StatusServiceUnavailable {
			resp.Body.Close()
			lastErr = fmt.Errorf("Browse %s: %s", objectID, resp.Status)
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(time.Duration(attempt+1) * 500 * time.Millisecond):
			}
			continue
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("Browse %s: %s", objectID, resp.Status)
		}
		return io.ReadAll(resp.Body)
	}
	return nil, lastErr
}

// browseContainer enumerates one container fully, following paging.
func (c *Client) browseContainer(ctx context.Context, objectID string) (items []Photo, containers []string, err error) {
	for start := 0; ; {
		raw, err := c.browse(ctx, objectID, start, browsePageSize)
		if err != nil {
			return nil, nil, err
		}
		its, cts, numReturned, total, err := parseBrowseResponse(raw)
		if err != nil {
			return nil, nil, err
		}
		items = append(items, its...)
		containers = append(containers, cts...)
		start += numReturned
		if numReturned == 0 || start >= total {
			return items, containers, nil
		}
	}
}

// browseConcurrency bounds simultaneous Browse requests. Sony's DLNA server is
// slow per call, so crawling containers concurrently turns a many-second
// sequential walk into a few seconds without overwhelming the camera.
const browseConcurrency = 6

// List enumerates every photo on the camera by crawling containers from the
// root concurrently, discovering the device on first use. Photos are returned
// sorted by filename for a stable order.
func (c *Client) List(ctx context.Context) ([]Photo, error) {
	if err := c.ensureService(ctx); err != nil {
		return nil, err
	}

	var (
		mu       sync.Mutex
		photos   []Photo
		seen     = map[string]bool{"0": true}
		wg       sync.WaitGroup
		sem      = make(chan struct{}, browseConcurrency)
		errOnce  sync.Once
		firstErr error
	)

	var crawl func(id string)
	crawl = func(id string) {
		defer wg.Done()
		if ctx.Err() != nil {
			return
		}
		sem <- struct{}{}
		items, children, err := c.browseContainer(ctx, id)
		<-sem
		if err != nil {
			errOnce.Do(func() { firstErr = err })
			return
		}
		mu.Lock()
		photos = append(photos, items...)
		var next []string
		for _, child := range children {
			if !seen[child] {
				seen[child] = true
				next = append(next, child)
			}
		}
		mu.Unlock()
		for _, child := range next {
			wg.Add(1)
			go crawl(child)
		}
	}

	wg.Add(1)
	go crawl("0")
	wg.Wait()

	if firstErr != nil {
		return nil, firstErr
	}
	sort.Slice(photos, func(i, j int) bool { return photos[i].Name < photos[j].Name })
	return photos, nil
}

func xmlEscape(s string) string {
	var b strings.Builder
	_ = xml.EscapeText(&b, []byte(s))
	return b.String()
}
