//! Sony PlayMemories / DLNA camera client.
//!
//! Talks to a Sony camera (e.g. the a6000) over its Wi-Fi access point: discover
//! the UPnP ContentDirectory, enumerate photos via `Browse`, and fetch image
//! bytes. All HTTP uses a hand-rolled blocking client with no socket options so
//! it runs on iSH/iOS (see `http`).

mod browse;
mod discover;
mod http;
mod model;

pub use browse::{BrowsePage, Container};
pub use model::Photo;

/// The DLNA device-description port Sony cameras serve on.
pub const DEFAULT_DESC_PORT: u16 = 64321;

/// A validated, connected camera target. Cheap to clone.
#[derive(Clone, Debug)]
pub struct Camera {
    host: String,
    friendly_name: Option<String>,
    control_url: String,
    service_type: String,
}

impl Camera {
    /// Connect to an explicit host, validating it by fetching+parsing its
    /// device description.
    pub fn connect(host: &str) -> Result<Camera, String> {
        let desc_url = format!("http://{host}:{DEFAULT_DESC_PORT}/DmsDesc.xml");
        Camera::from_desc_url(&desc_url)
    }

    /// Auto-discover a Sony camera (SSDP, then local-IP gateway probing) and
    /// connect to it.
    pub fn discover() -> Result<Camera, String> {
        let desc_url =
            discover::discover().ok_or_else(|| "no camera found via discovery".to_string())?;
        Camera::from_desc_url(&desc_url)
    }

    fn from_desc_url(desc_url: &str) -> Result<Camera, String> {
        let (bytes, _) = http::http_get_bytes(desc_url)?;
        let xml = String::from_utf8_lossy(&bytes);
        let (control_url, service_type) = browse::parse_device_description(&xml, desc_url)?;
        Ok(Camera {
            host: host_of(desc_url),
            friendly_name: browse::parse_friendly_name(&xml),
            control_url,
            service_type,
        })
    }

    /// The host this camera was reached at (e.g. "10.0.0.1").
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The camera's UPnP `friendlyName` (e.g. "ILCE-6000"), if it advertised one.
    pub fn friendly_name(&self) -> Option<&str> {
        self.friendly_name.as_deref()
    }

    /// A human label for the camera: its friendly name, else the host.
    pub fn label(&self) -> &str {
        self.friendly_name.as_deref().unwrap_or(&self.host)
    }

    /// Enumerate every photo on the camera.
    pub fn list(&self) -> Result<Vec<Photo>, String> {
        browse::list_all(&self.control_url, &self.service_type)
    }

    /// Browse one page `[start, start+count)` of a container's children — the
    /// building block the server's pager walks for offset/limit pagination.
    /// `container_id` "0" is the root.
    pub fn browse_children(
        &self,
        container_id: &str,
        start: usize,
        count: usize,
    ) -> Result<BrowsePage, String> {
        browse::browse_page(
            &self.control_url,
            &self.service_type,
            container_id,
            start,
            count,
        )
    }

    /// Fetch raw image bytes + content type for one of a photo's camera media
    /// URLs (for the server to proxy to the browser).
    pub fn fetch(&self, media_url: &str) -> Result<(Vec<u8>, String), String> {
        http::http_get_bytes(media_url)
    }
}

/// Extract the host from `http://host:port/path`.
fn host_of(url: &str) -> String {
    url.strip_prefix("http://")
        .and_then(|r| r.split('/').next())
        .map(|authority| authority.split(':').next().unwrap_or(authority).to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::host_of;

    #[test]
    fn host_of_strips_scheme_port_path() {
        assert_eq!(host_of("http://10.0.0.1:64321/DmsDesc.xml"), "10.0.0.1");
        assert_eq!(host_of("http://example/x"), "example");
    }
}
