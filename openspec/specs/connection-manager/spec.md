# connection-manager Specification

## Purpose
TBD - created by archiving change migrate-to-rust-product. Update Purpose after archive.
## Requirements
### Requirement: Server starts without a camera

The server SHALL start and serve the web UI regardless of whether a camera is
reachable. Camera connectivity SHALL NOT be a precondition for launch.

The server SHALL NOT connect to a camera on its own; all connecting is driven by
the web UI (there is no camera-host launch flag).

#### Scenario: Launch with no camera reachable

- **WHEN** the binary is started and no camera is connected
- **THEN** the HTTP server still binds `127.0.0.1` and serves the web UI at `/`
- **AND** it does not exit or block waiting for a camera

#### Scenario: Startup never contacts a camera

- **WHEN** the binary is started
- **THEN** no camera connection is attempted until the web UI calls
  `/api/connect`, so an unreachable camera never delays the server from listening

### Requirement: Connection state endpoint

The system SHALL expose `GET /api/state` returning the current connection status
as JSON, including whether a camera is connected, the active host (if any), the
last error (if any), and the number of photos currently known.

#### Scenario: State while disconnected

- **WHEN** a client requests `GET /api/state` with no camera connected
- **THEN** the response is `200` with `connected=false` and a `host` of null

#### Scenario: State while connected

- **WHEN** a camera is connected and the client requests `GET /api/state`
- **THEN** the response reports `connected=true` and the active `host`

### Requirement: Connect / change camera at runtime

The system SHALL expose `POST /api/connect` that sets the camera target. The body
MAY specify a `host`; if omitted, the server SHALL attempt auto-discovery. The
server SHALL validate the target by fetching and parsing its device description
BEFORE replacing the active target, so an invalid request does not drop an
existing good connection.

#### Scenario: Connect with an explicit IP

- **WHEN** the client POSTs `/api/connect` with `{"host":"10.0.0.1"}` and that
  host is a reachable Sony camera
- **THEN** the server validates it, makes it the active target, and returns state
  with `connected=true` and `host="10.0.0.1"`

#### Scenario: Connect with auto-discovery

- **WHEN** the client POSTs `/api/connect` with no host
- **THEN** the server runs discovery (SSDP, then local-IP gateway probing) and,
  if a Sony camera is found, connects to it

#### Scenario: Invalid host does not break the current connection

- **WHEN** a camera is already connected and the client POSTs `/api/connect` with
  an unreachable or non-Sony host
- **THEN** the server returns an error state describing the failure
- **AND** the previously active target remains connected and usable

### Requirement: Gallery routes reflect connection state

`GET /api/list` and the proxy routes SHALL operate against the current target.
When no camera is connected, `GET /api/list` SHALL return a structured
not-connected response (not a server crash) that the UI can act on.

#### Scenario: List while disconnected

- **WHEN** `GET /api/list` is requested with no camera connected
- **THEN** the response indicates "not connected" with a non-2xx status and a
  JSON body the UI uses to show the connect panel

### Requirement: Web UI connection flow

The web UI SHALL, on load, query connection state and attempt to connect; when
not connected it SHALL present a connect panel offering auto-retry and manual IP
entry, and SHALL provide a persistent control to change the camera / reconnect
mid-session.

#### Scenario: Auto-connect on load

- **WHEN** the page loads and a camera is reachable (or auto-discovery succeeds)
- **THEN** the gallery is shown without manual steps

#### Scenario: Manual IP entry when auto fails

- **WHEN** auto-connect fails
- **THEN** the UI shows a connect panel with an IP input and a connect button
- **AND** submitting a valid IP connects and loads the gallery

#### Scenario: Change camera mid-session

- **WHEN** the user opens the change-camera control and enters a different IP
- **THEN** the UI reconnects to that camera and refreshes the gallery

### Requirement: Connecting does not block other requests

The server SHALL handle requests concurrently so that an in-progress
`/api/connect` (including its discovery) does not delay `/api/state`, proxied
media, or static assets.

#### Scenario: State served during a slow connect

- **WHEN** a `/api/connect` is in progress and slow to complete
- **THEN** `/api/state`, thumbnail/photo requests, and the web UI assets are still
  served promptly, not queued behind the connect

### Requirement: Connect attempts are bounded and supersedable

A connect attempt SHALL fail within a bounded time rather than hang indefinitely
on an unreachable host, and a newer connect SHALL supersede an older in-flight
one so a stale result cannot overwrite newer connection state.

#### Scenario: Unreachable host fails fast

- **WHEN** discovery probes a candidate gateway that is routable but unresponsive
- **THEN** that probe times out in a couple of seconds and discovery continues,
  rather than blocking for tens of seconds

#### Scenario: Newer connect wins

- **WHEN** a connect is still in progress and a new connect request arrives
- **THEN** the newer attempt proceeds and, when the older attempt finishes, its
  result is discarded if it has been superseded (it does not clobber the state
  set by the newer attempt)

#### Scenario: Reuse an existing connection

- **WHEN** the camera is already connected and the client reloads
- **THEN** the existing connection is reused (no forced reconnect), reported via
  `/api/state`

### Requirement: Proxied media is browser-cacheable

`/api/thumb/:id` and `/api/photo/:id` responses SHALL carry caching headers so the
browser can serve a re-requested image from its own cache instead of re-fetching
it from the camera.

#### Scenario: Re-mounted thumbnail is not re-fetched

- **WHEN** a thumbnail that was already loaded is requested again (e.g. a
  virtualized tile scrolls back into view)
- **THEN** the browser serves it from cache, without another round-trip to the
  camera

