## ADDED Requirements

### Requirement: Discover the ContentDirectory service

The system SHALL locate the camera's UPnP ContentDirectory control URL by
fetching the device description from the camera and parsing it, so that no
hard-coded service URL is assumed across firmware variants.

The camera host SHALL default to `192.168.122.1` and SHALL be configurable.

#### Scenario: Parse a captured device description

- **WHEN** the client parses a captured `DmsDescPush.xml` fixture
- **THEN** it extracts the ContentDirectory `controlURL`, resolved to an absolute
  URL against the device base, and the SOAP `serviceType`

#### Scenario: Device description unreachable

- **WHEN** fetching the device description fails (host unreachable / timeout)
- **THEN** the client returns an error that names "not connected to the camera"
  rather than a raw socket error

### Requirement: Enumerate photos via UPnP Browse

The system SHALL enumerate photos by issuing a UPnP ContentDirectory `Browse`
SOAP action with `ObjectID=0` and `BrowseFlag=BrowseDirectChildren`, and SHALL
parse the returned DIDL-Lite result into a typed photo list.

Each photo SHALL expose: a stable `id`, a `name` (filename), a `date` when
available, a thumbnail URL, and an original (full) image URL, derived from the
item's `<res>` elements.

#### Scenario: Parse a captured Browse response

- **WHEN** the client parses a captured `Browse` DIDL-Lite response fixture with
  multiple photo items
- **THEN** it returns one typed photo per item with id, name, thumbnail URL and
  original URL populated

#### Scenario: Select res URLs by role

- **WHEN** an item exposes multiple `<res>` URLs (thumbnail, reduced, original)
- **THEN** the thumbnail URL maps to the smallest/thumbnail resolution and the
  full URL maps to the original JPEG, distinguished by their `protocolInfo` /
  resolution attributes

#### Scenario: Paged result set

- **WHEN** the camera returns results in pages (`NumberReturned` <
  `TotalMatches`)
- **THEN** the client issues further `Browse` calls advancing `StartingIndex`
  until all items are collected

### Requirement: Fetch image bytes

The system SHALL fetch thumbnail and original image bytes for a photo via plain
HTTP GET on the photo's `<res>` URLs, returning the bytes and content type for
streaming, without modifying the image.

#### Scenario: Fetch original returns JPEG bytes

- **WHEN** the client fetches the original URL for a photo
- **THEN** it returns the JPEG bytes and a `Content-Type` of `image/jpeg`

### Requirement: JPEG-only guarantee

The system SHALL NOT present RAW (`.ARW`) downloads over this path, because the
camera downscales RAW to JPEG when serving via "Select on Smartphone".

#### Scenario: RAW is never advertised

- **WHEN** building the typed photo list
- **THEN** every photo's full URL refers to a JPEG resource and no `.ARW`
  original is advertised
