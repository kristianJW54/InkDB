use crate::tree::btree::Blink;
// Layers
// B_inner - base of the b_tree used for traversal and algorithmic logic - coordinating operations
//
// Intuition
//
// Traversal:        Tree-level navigation → PageID
// Positioning:      Page-local logic → slot / found
// Modification:     Path-aware logic → splits & propagation
/*
    During traversal, Postgres may:
    - Follow right-links
    - Detect concurrent splits
    - Skip half-dead pages
    - Repair incomplete splits (sometimes lazily)
*/

pub(super) struct BInner<'blink> {
    tree: &'blink Blink,
}

impl<'blink> BInner<'blink> {
    pub fn new(tree: &'blink Blink) -> Self {
        Self { tree }
    }
}

// Need to have insertpath structure for paths
// Need to have leafpos
// Need to have cursor/scan
