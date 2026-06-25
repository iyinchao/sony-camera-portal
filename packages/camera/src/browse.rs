//! UPnP device-description parsing and DIDL-Lite `Browse` handling, plus the
//! recursive container crawl that collects every photo.

use crate::http::soap_browse;
use crate::model::Photo;
use std::collections::HashMap;

/// Items requested per Browse call when draining a container.
const PAGE: usize = 50;

/// Parse a device description, returning (absolute ContentDirectory controlURL,
/// serviceType).
pub(crate) fn parse_device_description(
    xml: &str,
    desc_url: &str,
) -> Result<(String, String), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    for service in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "service")
    {
        let mut stype = String::new();
        let mut ctrl = String::new();
        for c in service.children() {
            match c.tag_name().name() {
                "serviceType" => stype = c.text().unwrap_or("").trim().to_string(),
                "controlURL" => ctrl = c.text().unwrap_or("").trim().to_string(),
                _ => {}
            }
        }
        if stype.contains("ContentDirectory") && !ctrl.is_empty() {
            return Ok((resolve_url(desc_url, &ctrl), stype));
        }
    }
    Err("no ContentDirectory service in device description".into())
}

/// The device's `<friendlyName>` (e.g. "ILCE-6000") from its UPnP description.
pub(crate) fn parse_friendly_name(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    doc.descendants()
        .find(|n| n.tag_name().name() == "friendlyName")
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Resolve a possibly-relative controlURL against the description URL's origin.
fn resolve_url(base: &str, reference: &str) -> String {
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return reference.to_string();
    }
    let origin = match base.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.split('/').next().unwrap_or(rest);
            format!("{scheme}://{host}")
        }
        None => base.to_string(),
    };
    if reference.starts_with('/') {
        format!("{origin}{reference}")
    } else {
        format!("{origin}/{reference}")
    }
}

/// A child container in a Browse result.
#[derive(Clone, Debug)]
pub struct Container {
    pub id: String,
    pub title: String,
    pub child_count: Option<usize>,
}

/// One page of a container's children: photos + sub-containers, plus the paging
/// counters from the Browse response.
#[derive(Clone, Debug)]
pub struct BrowsePage {
    pub items: Vec<Photo>,
    pub containers: Vec<Container>,
    pub number_returned: usize,
    pub total_matches: usize,
}

fn parse_browse(soap_bytes: &[u8]) -> Result<BrowsePage, String> {
    let soap = String::from_utf8_lossy(soap_bytes);
    let doc = roxmltree::Document::parse(&soap).map_err(|e| e.to_string())?;

    let int_of = |name: &str| -> usize {
        doc.descendants()
            .find(|n| n.tag_name().name() == name)
            .and_then(|n| n.text())
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(0)
    };
    let number_returned = int_of("NumberReturned");
    let total_matches = int_of("TotalMatches");

    let result = doc
        .descendants()
        .find(|n| n.tag_name().name() == "Result")
        .and_then(|n| n.text())
        .unwrap_or("")
        .to_string();

    let mut items = Vec::new();
    let mut containers = Vec::new();
    if !result.trim().is_empty() {
        let ddoc = roxmltree::Document::parse(&result).map_err(|e| e.to_string())?;
        for node in ddoc.root_element().children().filter(|n| n.is_element()) {
            match node.tag_name().name() {
                "container" => {
                    let id = node.attribute("id").unwrap_or("").to_string();
                    if id.is_empty() {
                        continue;
                    }
                    let child_count = node
                        .attribute("childCount")
                        .and_then(|s| s.trim().parse().ok());
                    let title = node
                        .children()
                        .find(|c| c.tag_name().name() == "title")
                        .and_then(|c| c.text())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    containers.push(Container {
                        id,
                        title,
                        child_count,
                    });
                }
                "item" => {
                    let id = node.attribute("id").unwrap_or("").to_string();
                    let mut name = String::new();
                    let mut date = String::new();
                    let mut res: Vec<(String, String)> = Vec::new();
                    for c in node.children().filter(|n| n.is_element()) {
                        match c.tag_name().name() {
                            "title" => name = c.text().unwrap_or("").trim().to_string(),
                            "date" => date = c.text().unwrap_or("").trim().to_string(),
                            "res" => {
                                let proto = c.attribute("protocolInfo").unwrap_or("").to_string();
                                let url = c.text().unwrap_or("").trim().to_string();
                                if !url.is_empty() {
                                    res.push((proto, url));
                                }
                            }
                            _ => {}
                        }
                    }
                    let (thumb, full) = select_urls(&res);
                    items.push(Photo {
                        id,
                        name,
                        date,
                        thumb_url: thumb,
                        full_url: full,
                    });
                }
                _ => {}
            }
        }
    }
    Ok(BrowsePage {
        items,
        containers,
        number_returned,
        total_matches,
    })
}

/// Browse a single page `[start, start+count)` of a container's children.
pub(crate) fn browse_page(
    control_url: &str,
    service_type: &str,
    container_id: &str,
    start: usize,
    count: usize,
) -> Result<BrowsePage, String> {
    let raw = soap_browse(control_url, service_type, container_id, start, count)?;
    parse_browse(&raw)
}

/// Extract the DLNA.ORG_PN profile from a protocolInfo string ("" if absent).
fn dlna_pn(protocol_info: &str) -> &str {
    const KEY: &str = "DLNA.ORG_PN=";
    if let Some(i) = protocol_info.find(KEY) {
        let v = &protocol_info[i + KEY.len()..];
        let end = v.find([';', ':']).unwrap_or(v.len());
        &v[..end]
    } else {
        ""
    }
}

/// Pick thumbnail (JPEG_TN, falling back) and full-res original (the PN-less
/// res, falling back), by DLNA profile rather than position.
fn select_urls(res: &[(String, String)]) -> (String, String) {
    let mut by_pn: HashMap<&str, &str> = HashMap::new();
    let mut original = "";
    for (proto, url) in res {
        let pn = dlna_pn(proto);
        if pn.is_empty() {
            original = url;
        } else {
            by_pn.entry(pn).or_insert(url);
        }
    }
    let pick = |keys: &[&str]| -> String {
        for k in keys {
            if let Some(u) = by_pn.get(k) {
                return u.to_string();
            }
        }
        String::new()
    };
    let thumb = {
        let t = pick(&["JPEG_TN", "JPEG_SM", "JPEG_LRG"]);
        if t.is_empty() {
            original.to_string()
        } else {
            t
        }
    };
    let full = if !original.is_empty() {
        original.to_string()
    } else {
        pick(&["JPEG_LRG", "JPEG_SM", "JPEG_TN"])
    };
    (thumb, full)
}

/// Recursively browse from the root, collecting every photo (drains each
/// container, following paging).
pub(crate) fn list_all(control_url: &str, service_type: &str) -> Result<Vec<Photo>, String> {
    let mut photos = Vec::new();
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut queue = vec!["0".to_string()];
    while let Some(id) = queue.pop() {
        if seen.contains_key(&id) {
            continue;
        }
        seen.insert(id.clone(), ());
        let mut start = 0;
        loop {
            let page = browse_page(control_url, service_type, &id, start, PAGE)?;
            photos.extend(page.items);
            for c in &page.containers {
                if !seen.contains_key(&c.id) {
                    queue.push(c.id.clone());
                }
            }
            start += page.number_returned;
            if page.number_returned == 0 || start >= page.total_matches {
                break;
            }
        }
    }
    Ok(photos)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESC: &str = include_str!("../testdata/DmsDesc.xml");
    const BROWSE: &[u8] = include_bytes!("../testdata/browse_response.xml");

    #[test]
    fn parses_device_description() {
        let (ctrl, st) =
            parse_device_description(DESC, "http://10.0.0.1:64321/DmsDesc.xml").unwrap();
        assert_eq!(ctrl, "http://10.0.0.1:64321/upnp/control/ContentDirectory");
        assert_eq!(st, "urn:schemas-upnp-org:service:ContentDirectory:1");
    }

    #[test]
    fn parses_friendly_name() {
        assert_eq!(parse_friendly_name(DESC).as_deref(), Some("ILCE-6000"));
        assert_eq!(parse_friendly_name("<root/>"), None);
    }

    #[test]
    fn parses_browse_items() {
        let r = parse_browse(BROWSE).unwrap();
        assert_eq!(r.containers.len(), 0);
        assert_eq!(r.items.len(), 4);
        assert_eq!(r.number_returned, 4);
        assert_eq!(r.total_matches, 4);
        let p = &r.items[0];
        assert_eq!(p.id, "04_02_0326702136_000001_000001_000000");
        assert_eq!(p.name, "DSC07000.JPG");
        assert_eq!(p.date, "2014-01-01T00:00:10");
        assert!(p
            .thumb_url
            .starts_with("http://10.0.0.1:60151/TN_DSC07000.JPG"));
        assert!(p
            .full_url
            .starts_with("http://10.0.0.1:60151/ORG_DSC07000.JPG"));
    }

    #[test]
    fn dlna_pn_extracts_profile() {
        assert_eq!(
            dlna_pn("http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_TN;DLNA.ORG_CI=1"),
            "JPEG_TN"
        );
        assert_eq!(dlna_pn("http-get:*:image/jpeg:*"), "");
    }

    #[test]
    fn select_urls_prefers_tn_and_original() {
        let res = vec![
            (
                "http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_LRG".into(),
                "http://h/LRG.JPG".into(),
            ),
            (
                "http-get:*:image/jpeg:DLNA.ORG_PN=JPEG_TN".into(),
                "http://h/TN.JPG".into(),
            ),
            ("http-get:*:image/jpeg:*".into(), "http://h/ORG.JPG".into()),
        ];
        let (thumb, full) = select_urls(&res);
        assert_eq!(thumb, "http://h/TN.JPG");
        assert_eq!(full, "http://h/ORG.JPG");
    }

    #[test]
    fn parses_container_list_with_childcount() {
        // A grouping Browse: child containers carrying childCount + date titles.
        let soap = "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body>\
<u:BrowseResponse xmlns:u=\"urn:schemas-upnp-org:service:ContentDirectory:1\">\
<Result>&lt;DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\"&gt;\
&lt;container id=\"d1\" childCount=\"6\"&gt;&lt;dc:title&gt;2026-06-01&lt;/dc:title&gt;&lt;/container&gt;\
&lt;container id=\"d2\" childCount=\"4\"&gt;&lt;dc:title&gt;2026-06-02&lt;/dc:title&gt;&lt;/container&gt;\
&lt;/DIDL-Lite&gt;</Result><NumberReturned>2</NumberReturned><TotalMatches>2</TotalMatches>\
</u:BrowseResponse></s:Body></s:Envelope>";
        let r = parse_browse(soap.as_bytes()).unwrap();
        assert_eq!(r.items.len(), 0);
        assert_eq!(r.total_matches, 2);
        assert_eq!(r.containers.len(), 2);
        assert_eq!(r.containers[0].id, "d1");
        assert_eq!(r.containers[0].title, "2026-06-01");
        assert_eq!(r.containers[0].child_count, Some(6));
        assert_eq!(r.containers[1].child_count, Some(4));
    }
}
