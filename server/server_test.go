package server

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/iyinchao/sony-camera-portal/camera"
)

// stubCam implements Camera without a real device.
type stubCam struct {
	photos  []camera.Photo
	listErr error
	open    func(mediaURL string) (io.ReadCloser, string, error)
}

func (s *stubCam) List(context.Context) ([]camera.Photo, error) {
	return s.photos, s.listErr
}

func (s *stubCam) Open(_ context.Context, mediaURL string) (io.ReadCloser, string, error) {
	return s.open(mediaURL)
}

func newTestServer(cam Camera) *Server {
	static := fstest.MapFS{
		"index.html": {Data: []byte("<!doctype html><title>gallery</title>")},
	}
	return New(cam, static)
}

func TestList(t *testing.T) {
	cam := &stubCam{photos: []camera.Photo{
		{ID: "04_02_x", Name: "DSC07000.JPG", Date: "2014-01-01T00:00:10",
			ThumbURL: "http://10.0.0.1:60151/TN_DSC07000.JPG",
			FullURL:  "http://10.0.0.1:60151/ORG_DSC07000.JPG"},
	}}
	srv := newTestServer(cam)

	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest("GET", "/api/list", nil))

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rr.Code)
	}
	var got []apiPhoto
	if err := json.Unmarshal(rr.Body.Bytes(), &got); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if len(got) != 1 {
		t.Fatalf("len = %d, want 1", len(got))
	}
	p := got[0]
	if p.ThumbURL != "/api/thumb/04_02_x" {
		t.Errorf("thumbUrl = %q, want proxied /api/thumb/...", p.ThumbURL)
	}
	if p.FullURL != "/api/photo/04_02_x" {
		t.Errorf("fullUrl = %q, want proxied /api/photo/...", p.FullURL)
	}
	if p.Name != "DSC07000.JPG" {
		t.Errorf("name = %q", p.Name)
	}
}

func TestListNotConnected(t *testing.T) {
	srv := newTestServer(&stubCam{listErr: camera.ErrNotConnected})

	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest("GET", "/api/list", nil))

	if rr.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503", rr.Code)
	}
	var body map[string]string
	if err := json.Unmarshal(rr.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if body["error"] == "" {
		t.Error("expected a non-empty error message")
	}
}

func TestThumbProxy(t *testing.T) {
	var requested string
	cam := &stubCam{
		photos: []camera.Photo{{ID: "id1", Name: "A.JPG",
			ThumbURL: "http://cam/TN.JPG", FullURL: "http://cam/ORG.JPG"}},
		open: func(u string) (io.ReadCloser, string, error) {
			requested = u
			return io.NopCloser(strings.NewReader("JPEGBYTES")), "image/jpeg", nil
		},
	}
	srv := newTestServer(cam)

	// Populate the id->url map first.
	srv.ServeHTTP(httptest.NewRecorder(), httptest.NewRequest("GET", "/api/list", nil))

	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest("GET", "/api/thumb/id1", nil))

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rr.Code)
	}
	if requested != "http://cam/TN.JPG" {
		t.Errorf("proxied %q, want the thumbnail URL", requested)
	}
	if ct := rr.Header().Get("Content-Type"); ct != "image/jpeg" {
		t.Errorf("content-type = %q", ct)
	}
	if rr.Body.String() != "JPEGBYTES" {
		t.Errorf("body = %q", rr.Body.String())
	}
}

func TestPhotoIsAttachment(t *testing.T) {
	cam := &stubCam{
		photos: []camera.Photo{{ID: "id1", Name: "DSC07000.JPG",
			ThumbURL: "http://cam/TN.JPG", FullURL: "http://cam/ORG.JPG"}},
		open: func(u string) (io.ReadCloser, string, error) {
			return io.NopCloser(strings.NewReader("ORIG")), "image/jpeg", nil
		},
	}
	srv := newTestServer(cam)
	srv.ServeHTTP(httptest.NewRecorder(), httptest.NewRequest("GET", "/api/list", nil))

	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest("GET", "/api/photo/id1", nil))

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rr.Code)
	}
	cd := rr.Header().Get("Content-Disposition")
	if !strings.Contains(cd, "attachment") || !strings.Contains(cd, "DSC07000.JPG") {
		t.Errorf("Content-Disposition = %q, want attachment with filename", cd)
	}
}

func TestUnknownID(t *testing.T) {
	srv := newTestServer(&stubCam{
		open: func(string) (io.ReadCloser, string, error) {
			t.Fatal("Open should not be called for an unknown id")
			return nil, "", nil
		},
	})
	for _, path := range []string{"/api/thumb/nope", "/api/photo/nope"} {
		rr := httptest.NewRecorder()
		srv.ServeHTTP(rr, httptest.NewRequest("GET", path, nil))
		if rr.Code != http.StatusNotFound {
			t.Errorf("%s status = %d, want 404", path, rr.Code)
		}
	}
}

func TestServesEmbeddedUI(t *testing.T) {
	srv := newTestServer(&stubCam{})
	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest("GET", "/", nil))

	if rr.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rr.Code)
	}
	if !strings.Contains(rr.Body.String(), "gallery") {
		t.Errorf("index body = %q, want embedded UI", rr.Body.String())
	}
}
