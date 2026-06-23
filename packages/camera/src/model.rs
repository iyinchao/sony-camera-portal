//! The typed photo model.

/// One image enumerated from the camera.
///
/// `thumb_url` / `full_url` are the camera's own media URLs; the server proxies
/// them so the browser never contacts the camera directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Photo {
    pub id: String,
    pub name: String,
    pub date: String,
    pub thumb_url: String,
    pub full_url: String,
}
