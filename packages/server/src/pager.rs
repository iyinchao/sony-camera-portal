//! Lazy, paginated walk over the camera's container tree.
//!
//! Photos live in ~30 date containers under a container-only spine
//! (`0 → PhotoRoot → grouping → [date containers] → items`). The pager resolves
//! that spine once to get the ordered leaf containers, then serves
//! `[offset, offset+limit)` by browsing only the leaves the page touches —
//! caching items/counts so the next page doesn't re-walk. Returns the first page
//! after a few Browse calls instead of enumerating the whole library.

use camera::{Camera, Photo};

/// Items requested per Browse call.
const PAGE: usize = 50;

/// One page of photos plus paging metadata.
pub struct Page {
    pub photos: Vec<Photo>,
    pub total: Option<usize>,
    pub has_more: bool,
}

struct Leaf {
    id: String,
    /// Known item count (from `childCount`, or learned after browsing).
    count: Option<usize>,
    /// Items cached once the leaf has been loaded.
    items: Vec<Photo>,
    loaded: bool,
}

pub struct Pager {
    leaves: Vec<Leaf>,
    resolved: bool,
}

impl Default for Pager {
    fn default() -> Self {
        Self::new()
    }
}

impl Pager {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            resolved: false,
        }
    }

    /// Serve photos `[offset, offset+limit)`.
    pub fn page(&mut self, cam: &Camera, offset: usize, limit: usize) -> Result<Page, String> {
        if !self.resolved {
            self.resolve(cam)?;
        }

        let mut out: Vec<Photo> = Vec::new();
        let mut base = 0usize; // running count of items before the current leaf
        let mut i = 0;
        while i < self.leaves.len() && out.len() < limit {
            // Need this leaf's count to place it on the global axis.
            let count = match self.leaves[i].count {
                Some(c) => c,
                None => {
                    self.load_leaf(cam, i)?;
                    self.leaves[i].count.unwrap_or(0)
                }
            };
            let leaf_end = base + count;

            if leaf_end <= offset {
                // Entirely before the window — skip cheaply.
                base = leaf_end;
                i += 1;
                continue;
            }
            // Overlaps the window — ensure items are loaded, then take the slice.
            if !self.leaves[i].loaded {
                self.load_leaf(cam, i)?;
            }
            let items = &self.leaves[i].items;
            let from = offset.saturating_sub(base).min(items.len());
            let want = limit - out.len();
            let to = (from + want).min(items.len());
            out.extend_from_slice(&items[from..to]);

            base = leaf_end;
            i += 1;
        }

        let total = if self.leaves.iter().all(|l| l.count.is_some()) {
            Some(self.leaves.iter().map(|l| l.count.unwrap_or(0)).sum())
        } else {
            None
        };
        // With a known total, has_more is exact; otherwise assume more whenever we
        // filled the page (a short page signals the end).
        let has_more = match total {
            Some(t) => offset + out.len() < t,
            None => out.len() >= limit,
        };

        Ok(Page {
            photos: out,
            total,
            has_more,
        })
    }

    /// Descend the container-only spine to the ordered leaf containers.
    fn resolve(&mut self, cam: &Camera) -> Result<(), String> {
        let mut current = "0".to_string();
        loop {
            let page = cam.browse_children(&current, 0, PAGE)?;
            if !page.items.is_empty() {
                // `current` itself holds items — a single leaf.
                self.leaves = vec![Leaf {
                    id: current,
                    count: Some(page.total_matches),
                    items: Vec::new(),
                    loaded: false,
                }];
                break;
            }
            if page.containers.len() == 1 {
                current = page.containers[0].id.clone();
                continue;
            }
            // Branching (or empty): treat these containers as the leaves.
            self.leaves = page
                .containers
                .iter()
                .map(|c| Leaf {
                    id: c.id.clone(),
                    count: c.child_count,
                    items: Vec::new(),
                    loaded: false,
                })
                .collect();
            break;
        }
        self.resolved = true;
        Ok(())
    }

    /// Fully load one leaf's items (following its own paging). If the "leaf"
    /// turns out to be intermediate (only sub-containers), splice its children
    /// into the list and load the first of them instead.
    fn load_leaf(&mut self, cam: &Camera, i: usize) -> Result<(), String> {
        let id = self.leaves[i].id.clone();
        let mut items = Vec::new();
        let mut subs = Vec::new();
        let mut start = 0;
        loop {
            let page = cam.browse_children(&id, start, PAGE)?;
            items.extend(page.items);
            subs.extend(page.containers);
            start += page.number_returned;
            if page.number_returned == 0 || start >= page.total_matches {
                break;
            }
        }

        if items.is_empty() && !subs.is_empty() {
            let replacement: Vec<Leaf> = subs
                .iter()
                .map(|c| Leaf {
                    id: c.id.clone(),
                    count: c.child_count,
                    items: Vec::new(),
                    loaded: false,
                })
                .collect();
            self.leaves.splice(i..=i, replacement);
            return self.load_leaf(cam, i);
        }

        self.leaves[i].count = Some(items.len());
        self.leaves[i].items = items;
        self.leaves[i].loaded = true;
        Ok(())
    }
}
