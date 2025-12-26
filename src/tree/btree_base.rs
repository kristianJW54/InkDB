use crate::page::PageID;
use crate::page::internal_page::IndexPageError;
use crate::transaction::tx_memory::TxMemory;
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

// NOTES:
// B-tree owns the split logic: Calls into page specific layer to handle keys etc which in turn calls into slotted_page to get bytes and size etc
// Need a SplitStrategy struct? separate file within this folder?

pub(super) type Result<T> = std::result::Result<T, BTreeInnerError>;

pub(super) enum BTreeInnerError {
    // Define error variants here
    IndexPageError(IndexPageError),
}

impl From<IndexPageError> for BTreeInnerError {
    fn from(err: IndexPageError) -> Self {
        BTreeInnerError::IndexPageError(err)
    }
}

pub(super) struct BInner<'blink> {
    tx: &'blink TxMemory,
}

impl<'blink> BInner<'blink> {
    pub fn new(tx: &'blink TxMemory) -> Self {
        Self { tx }
    }

    pub(super) fn traverse(&self, page: PageID, key: &[u8]) -> Result<PageID> {
        // Traversal assumes that the calling B-tree has fetched the root/fast root from the meta page and hands
        // over the page ID to start traversal from.

        // We want to traverse down on the key starting from the page

        Ok(PageID(0))
    }
}

// Need to have insertpath structure for paths - which we can pass to a traverse_with_path?
// Need to have leafpos - basically slot entry for the leaf page?
// Need to have cursor/scan - for horizontal movement?
