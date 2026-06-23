package main

import (
	"bytes"
	"context"
	"fmt"
	"image"
	"image/color"
	"image/jpeg"
	"io"
	"strconv"
	"strings"

	"github.com/iyinchao/sony-camera-portal/camera"
)

// mockCamera implements server.Camera without a real device, generating
// colored placeholder JPEGs on the fly. It exists so the UI (grid, multi-select,
// download) can be exercised offline with `--mock N`.
type mockCamera struct {
	photos []camera.Photo
}

func newMockCamera(n int) *mockCamera {
	m := &mockCamera{}
	for i := 0; i < n; i++ {
		m.photos = append(m.photos, camera.Photo{
			ID:       fmt.Sprintf("mock-%03d", i),
			Name:     fmt.Sprintf("DSC%05d.JPG", 7000+i),
			Date:     fmt.Sprintf("2026-06-%02dT%02d:00:00", 1+i/6, i%24), // ~6 per day → visible date groups
			ThumbURL: fmt.Sprintf("mock:thumb:%d", i),
			FullURL:  fmt.Sprintf("mock:full:%d", i),
		})
	}
	return m
}

func (m *mockCamera) List(context.Context) ([]camera.Photo, error) {
	return m.photos, nil
}

func (m *mockCamera) Open(_ context.Context, mediaURL string) (io.ReadCloser, string, error) {
	kind, idx, err := parseMockURL(mediaURL)
	if err != nil {
		return nil, "", err
	}
	w, h := 320, 213 // thumbnail
	if kind == "full" {
		w, h = 1200, 800 // "original"
	}
	data := placeholderJPEG(w, h, hue(idx))
	return io.NopCloser(bytes.NewReader(data)), "image/jpeg", nil
}

func parseMockURL(u string) (kind string, idx int, err error) {
	parts := strings.Split(u, ":") // mock:thumb:3
	if len(parts) != 3 || parts[0] != "mock" {
		return "", 0, fmt.Errorf("bad mock url %q", u)
	}
	idx, err = strconv.Atoi(parts[2])
	return parts[1], idx, err
}

// placeholderJPEG renders a simple two-tone tile so adjacent photos look
// distinct in the grid.
func placeholderJPEG(w, h int, c color.RGBA) []byte {
	img := image.NewRGBA(image.Rect(0, 0, w, h))
	darker := color.RGBA{c.R / 2, c.G / 2, c.B / 2, 255}
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			// diagonal split for a bit of visual variety
			if x+y < (w+h)/2 {
				img.Set(x, y, c)
			} else {
				img.Set(x, y, darker)
			}
		}
	}
	var buf bytes.Buffer
	_ = jpeg.Encode(&buf, img, &jpeg.Options{Quality: 80})
	return buf.Bytes()
}

// hue spreads colors around the wheel by index.
func hue(i int) color.RGBA {
	switch i % 6 {
	case 0:
		return color.RGBA{0xe7, 0x4c, 0x3c, 255} // red
	case 1:
		return color.RGBA{0xe6, 0x7e, 0x22, 255} // orange
	case 2:
		return color.RGBA{0xf1, 0xc4, 0x0f, 255} // yellow
	case 3:
		return color.RGBA{0x2e, 0xcc, 0x71, 255} // green
	case 4:
		return color.RGBA{0x34, 0x98, 0xdb, 255} // blue
	default:
		return color.RGBA{0x9b, 0x59, 0xb6, 255} // purple
	}
}
