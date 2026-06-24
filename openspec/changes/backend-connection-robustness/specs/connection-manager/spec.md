## ADDED Requirements

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
