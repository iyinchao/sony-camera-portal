// Package server exposes the local HTTP gallery: it serves the embedded web UI
// and proxies /api routes to a camera, so the browser stays same-origin on
// localhost and never contacts the camera (which CORS / mixed-content would
// block) directly.
package server

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"log"
	"net/http"
	"net/url"
	"sync"

	"github.com/iyinchao/sony-camera-portal/camera"
)

// Camera is the slice of the camera client the server needs. *camera.Client
// satisfies it; tests pass a stub.
type Camera interface {
	List(ctx context.Context) ([]camera.Photo, error)
	Open(ctx context.Context, mediaURL string) (body io.ReadCloser, contentType string, err error)
}

// Server is an http.Handler serving the UI and the /api proxy.
type Server struct {
	cam Camera
	mux *http.ServeMux

	mu     sync.RWMutex
	photos map[string]camera.Photo // id -> camera-side photo, rebuilt each /api/list
}

// apiPhoto is the /api/list item. Its URLs point at the server's own proxy
// routes, never the camera.
type apiPhoto struct {
	ID       string `json:"id"`
	Name     string `json:"name"`
	Date     string `json:"date"`
	ThumbURL string `json:"thumbUrl"`
	FullURL  string `json:"fullUrl"`
}

// New builds a server proxying to cam and serving static from staticFS (the
// embedded web/dist).
func New(cam Camera, staticFS fs.FS) *Server {
	s := &Server{
		cam:    cam,
		mux:    http.NewServeMux(),
		photos: map[string]camera.Photo{},
	}
	s.mux.HandleFunc("GET /api/list", s.handleList)
	s.mux.HandleFunc("GET /api/thumb/{id}", s.handleThumb)
	s.mux.HandleFunc("GET /api/photo/{id}", s.handlePhoto)
	s.mux.Handle("/", http.FileServerFS(staticFS))
	return s
}

func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	s.mux.ServeHTTP(w, r)
}

func (s *Server) handleList(w http.ResponseWriter, r *http.Request) {
	photos, err := s.cam.List(r.Context())
	if err != nil {
		writeError(w, err)
		return
	}

	index := make(map[string]camera.Photo, len(photos))
	out := make([]apiPhoto, 0, len(photos))
	for _, p := range photos {
		index[p.ID] = p
		out = append(out, apiPhoto{
			ID:       p.ID,
			Name:     p.Name,
			Date:     p.Date,
			ThumbURL: "/api/thumb/" + url.PathEscape(p.ID),
			FullURL:  "/api/photo/" + url.PathEscape(p.ID),
		})
	}

	s.mu.Lock()
	s.photos = index
	s.mu.Unlock()

	writeJSON(w, out)
}

func (s *Server) handleThumb(w http.ResponseWriter, r *http.Request) {
	p, ok := s.lookup(r.PathValue("id"))
	if !ok {
		http.NotFound(w, r)
		return
	}
	s.proxy(w, r, p.ThumbURL)
}

func (s *Server) handlePhoto(w http.ResponseWriter, r *http.Request) {
	p, ok := s.lookup(r.PathValue("id"))
	if !ok {
		http.NotFound(w, r)
		return
	}
	// Force a download with the camera's filename.
	w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%q", p.Name))
	s.proxy(w, r, p.FullURL)
}

func (s *Server) lookup(id string) (camera.Photo, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	p, ok := s.photos[id]
	return p, ok
}

// proxy streams a camera media URL to the client.
func (s *Server) proxy(w http.ResponseWriter, r *http.Request, mediaURL string) {
	body, contentType, err := s.cam.Open(r.Context(), mediaURL)
	if err != nil {
		writeError(w, err)
		return
	}
	defer body.Close()
	w.Header().Set("Content-Type", contentType)
	if _, err := io.Copy(w, body); err != nil {
		log.Printf("proxy %s: %v", mediaURL, err)
	}
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	if err := json.NewEncoder(w).Encode(v); err != nil {
		log.Printf("encode response: %v", err)
	}
}

// writeError maps camera failures to a status + JSON {"error": ...}. A
// not-connected error is a 503 with a user-actionable hint.
func writeError(w http.ResponseWriter, err error) {
	status := http.StatusBadGateway
	msg := err.Error()
	if errors.Is(err, camera.ErrNotConnected) {
		status = http.StatusServiceUnavailable
		msg = "Can’t reach the camera. Connect to its Wi-Fi (Send to Smartphone → Select on Smartphone)."
	}
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]string{"error": msg})
}
