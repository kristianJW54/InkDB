use crate::buffer::buffer_manager::BufferManagerError;
use crate::buffer::page_frame::PageFrameError;
use crate::operation::op_ctx::OpCtx;
use crate::page::internal_page::{InternalPageError, InternalPageRef};
use crate::page::{IndexLevel, PageID, PageKind, SlottedPageMut, SlottedPageRef};

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

#[derive(Debug)]
pub(super) enum BTreeInnerError {
    // Define error variants here
    BufferManagerError(BufferManagerError),
    IndexPageError(InternalPageError),
    PageFrameError(PageFrameError),
    TraverseError(PageID, Option<PageKind>), // Would want to format this error message
}

impl From<InternalPageError> for BTreeInnerError {
    fn from(err: InternalPageError) -> Self {
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

pub(super) struct BInner<'blink> {
    tx: &'blink OpCtx,
}

// TODO: Need to implement an insert method which returns a Result enum of InsertResult which is either a Ok or Split - Can we use null pointer optimisation?

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
            let page_handle = self.tx.pager.fetch_page_read(page_id)?;
            println!("on page id {:?}", page_id);

            match page_handle.page_kind() {
                PageKind::IndexInternal => {
                    // We are still in internal and so must traverse down
                    page_id = page_handle.with_read(|page| {
                        let sp = SlottedPageRef::from_bytes(page);
                        let internal_page = InternalPageRef::from_slotted_page(sp);
                        internal_page.find_child_ptr(key)
                    })?;

                    println!("moved to {:?}", page_id);
                }
                PageKind::IndexLeaf => {
                    // We have reached the leaf page and must return the page id
                    return Ok(page_id);
                }
                _ => {
                    return Err(BTreeInnerError::TraverseError(
                        page_id,
                        Some(page_handle.page_kind()),
                    ));
                }
            }
        }
    }

    pub(super) fn traverse_to_leaf_with_ctx<'a>(
        &self,
        page: PageID,
        key: &'a [u8],
    ) -> Result<TraverseCtx<'a>> {
        // Need to set up a stack which we will use to keep track of the traversal path
        let mut trav_ctx = TraverseCtx::from_key(key);

        let mut page_id = page;

        loop {
            let page_handle = self.tx.pager.fetch_page_read(page_id)?;
            trav_ctx.stack.push(page_id);
            match page_handle.page_kind() {
                PageKind::IndexInternal => {
                    let child_ptr = page_handle.with_read(|page| {
                        let sp = SlottedPageRef::from_bytes(page);
                        let internal = InternalPageRef::from_slotted_page(sp);
                        internal.find_child_ptr(key)
                    })?;
                    page_id = child_ptr;
                }
                PageKind::IndexLeaf => {
                    return Ok(trav_ctx);
                }
                _ => {
                    return Err(BTreeInnerError::TraverseError(
                        page_id,
                        Some(page_handle.page_kind()),
                    ));
                }
            }
        }
    }

    pub(super) fn try_insert(&mut self, key: &[u8]) -> Result<()> {
        //

        todo!("finish insert")
    }
}

pub(super) struct TraverseCtx<'a> {
    key: &'a [u8],
    stack: Vec<PageID>,
}

impl<'a> TraverseCtx<'a> {
    pub(super) fn from_key(key: &'a [u8]) -> Self {
        Self {
            key,
            stack: Vec::new(),
        }
    }
}

// Need to have insertpath structure for paths - which we can pass to a traverse_with_path?
// Need to have cursor/scan - for horizontal movement - scan cursor should logically sit between pages so splits will not interrupt scans and we can continue

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_time() {}
}
