## ADDED Requirements

### Requirement: Localhost-only HTTP server

The system SHALL run an HTTP server that binds `127.0.0.1` only (never
`0.0.0.0`), on a configurable port, and SHALL print the resulting localhost URL
on startup.

#### Scenario: Bind address is loopback

- **WHEN** the server starts
- **THEN** it listens on `127.0.0.1:<port>` and is not reachable from other
  hosts on the network

#### Scenario: Port is configurable

- **WHEN** the user passes `--port <n>`
- **THEN** the server listens on that port and prints `http://127.0.0.1:<n>`

### Requirement: Embedded web UI

The system SHALL embed the `web/` frontend into the binary via `go:embed` and
serve it at `/`, so the program is a single self-contained binary with no
external asset files at runtime.

#### Scenario: Serve the embedded index

- **WHEN** a browser requests `GET /`
- **THEN** the server responds with the embedded gallery HTML and its embedded
  assets, with no filesystem dependency

### Requirement: Photo list API

The system SHALL expose `GET /api/list` returning a JSON array of
`{ id, name, date, thumbUrl, fullUrl }`, where `thumbUrl` and `fullUrl` point at
the server's own proxy routes (not the camera directly).

#### Scenario: List returns proxied URLs

- **WHEN** a client requests `GET /api/list` while the camera is reachable
- **THEN** the server returns a JSON array whose `thumbUrl` is `/api/thumb/:id`
  and whose `fullUrl` is `/api/photo/:id` for each photo

#### Scenario: Camera not connected

- **WHEN** the client is not joined to the camera AP and requests `GET /api/list`
- **THEN** the server responds with a non-200 status and a JSON error body whose
  message tells the user to connect to the camera's Wi-Fi

### Requirement: Thumbnail and original proxy routes

The system SHALL proxy image bytes from the camera through
`GET /api/thumb/:id` (thumbnail) and `GET /api/photo/:id` (original JPEG),
fetching server-side so the browser never contacts the camera directly. The
`/api/photo/:id` response SHALL set `Content-Disposition: attachment` with the
photo's filename.

#### Scenario: Thumbnail proxy streams bytes

- **WHEN** a browser requests `GET /api/thumb/:id` for a known id
- **THEN** the server fetches the thumbnail from the camera and streams the bytes
  with `Content-Type: image/jpeg`

#### Scenario: Original download is an attachment

- **WHEN** a browser requests `GET /api/photo/:id` for a known id
- **THEN** the server streams the original JPEG with
  `Content-Disposition: attachment; filename="<name>.JPG"`

#### Scenario: Unknown id

- **WHEN** a request targets `/api/thumb/:id` or `/api/photo/:id` with an id not
  in the current listing
- **THEN** the server responds `404`
