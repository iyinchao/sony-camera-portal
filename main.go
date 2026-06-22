// Command sony-camera-portal runs a local web gallery for browsing and
// downloading JPEGs from a Sony camera over its Wi-Fi access point.
package main

import (
	"embed"
	"flag"
	"fmt"
	"io/fs"
	"log"
	"net"
	"net/http"
	"os/exec"
	"runtime"

	"github.com/iyinchao/sony-camera-portal/camera"
	"github.com/iyinchao/sony-camera-portal/server"
)

// web/dist is produced by `cd web && npm run build` and embedded here, so the
// binary is fully self-contained and offline.
//
//go:embed all:web/dist
var webDist embed.FS

func main() {
	port := flag.Int("port", 8080, "localhost port to listen on")
	host := flag.String("camera-host", "", "camera host/IP (default: SSDP-discover, fallback "+camera.DefaultHost+")")
	noOpen := flag.Bool("no-open", false, "do not auto-open the browser")
	mock := flag.Int("mock", 0, "serve N fake photos instead of a real camera (UI testing, no camera needed)")
	flag.Parse()

	dist, err := fs.Sub(webDist, "web/dist")
	if err != nil {
		log.Fatalf("embedded web assets: %v", err)
	}

	var cam server.Camera = &camera.Client{Host: *host}
	if *mock > 0 {
		log.Printf("mock mode: serving %d fake photos (no camera)", *mock)
		cam = newMockCamera(*mock)
	}
	srv := server.New(cam, dist)

	// Bind 127.0.0.1 only — never expose the camera proxy to the LAN.
	addr := fmt.Sprintf("127.0.0.1:%d", *port)
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		log.Fatalf("listen on %s: %v", addr, err)
	}
	u := "http://" + addr
	log.Printf("sony-camera-portal listening at %s", u)

	if !*noOpen {
		openBrowser(u)
	}
	log.Fatal(http.Serve(ln, srv))
}

// openBrowser best-effort opens the default browser on desktop platforms. On
// Android (Termux) there is no default GUI browser, so the printed URL is the
// entry point.
func openBrowser(u string) {
	var cmd string
	var args []string
	switch runtime.GOOS {
	case "darwin":
		cmd, args = "open", []string{u}
	case "windows":
		cmd, args = "rundll32", []string{"url.dll,FileProtocolHandler", u}
	case "linux":
		cmd, args = "xdg-open", []string{u}
	default:
		return // android/termux and others: just print the URL
	}
	if err := exec.Command(cmd, args...).Start(); err != nil {
		log.Printf("could not open browser (%v); visit %s manually", err, u)
	}
}
