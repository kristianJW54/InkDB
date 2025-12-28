use crate::buffer::buffer_manager::BufferManagerError;
use crate::buffer::page_frame::PageFrameError;
use crate::operation::op_ctx::OpCtx;
use crate::page::internal_page::{IndexPageError, IndexPageRef};
use crate::page::{PageID, PageKind, SlottedPageMut, SlottedPageRef};

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
    BufferManagerError(BufferManagerError),
    IndexPageError(IndexPageError),
    PageFrameError(PageFrameError),
    TraverseError,
}

impl From<IndexPageError> for BTreeInnerError {
    fn from(err: IndexPageError) -> Self {
        BTreeInnerError::IndexPageError(err)
    }
}

impl From<BufferManagerError> for BTreeInnerError {
    fn from(err: BufferManagerError) -> Self {
        BTreeInnerError::BufferManagerError(err)
    }
}

impl From<PageFrameError> for BTreeInnerError {
    fn from(err: PageFrameError) -> Self {
        BTreeInnerError::PageFrameError(err)
    }
}

// TOOD Think more on this if we need it
struct TraverseCtx<'a> {
    key: &'a [u8],
    level: u8,
    stack: Vec<PageID>,
}

pub(super) struct BInner<'blink> {
    tx: &'blink OpCtx,
}

impl<'blink> BInner<'blink> {
    pub fn new(tx: &'blink OpCtx) -> Self {
        Self { tx }
    }

    pub(super) fn traverse_to_leaf(&self, page: PageID, key: &[u8]) -> Result<PageID> {
        // Traversal assumes that the calling B-tree has fetched the root/fast root from the meta page and hands
        // over the page ID to start traversal from.

        let mut page_id = page;

        loop {
            // Each iteration we need to fetch the page and then match on the page kind
            let mut page_handle = self.tx.pager.fetch_page_read(page)?;

            match page_handle.page_kind() {
                PageKind::IndexInternal => {
                    // We are still in internal and so must traverse down
                    page_id = page_handle.with_read(|page| {
                        let sp = SlottedPageRef::from_bytes(page);
                        let internal_page = IndexPageRef::from_slotted_page(sp);
                        internal_page.find_child_ptr(key)
                    })?;
                }
                PageKind::IndexLeaf => {
                    // We have reached the leaf page and must return the page id
                    return Ok(page_handle.page_id());
                }
                _ => {
                    break;
                }
            }
        }

        // We need to loop until we error or are at leaf page

        // We want to traverse down on the key starting from the page

        Ok(PageID(0))
    }
}

// Need to have insertpath structure for paths - which we can pass to a traverse_with_path?
// Need to have leafpos - basically slot entry for the leaf page this is only when we work inside the leaf page and should not be used outside of latch
// Need to have cursor/scan - for horizontal movement - scan cursor should logically sit between pages so splits will not interrupt scans and we can continue

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_traversal() {

        // First setup the page manager
        // Add in a few pages with content
        // Setup OpCtx
        // Make a new tree
        // Traverse to find leaf
    }
}
