use crate::buffer::buffer_manager::BufferManagerError;
use crate::buffer::page_frame::PageFrameError;
use crate::operation::op_ctx::OpCtx;
use crate::page::internal_page::{IndexPageError, IndexPageRef};
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
    IndexPageError(IndexPageError),
    PageFrameError(PageFrameError),
    TraverseError(PageID, Option<PageKind>), // Would want to format this error message
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
pub(super) struct TraverseCtx<'a> {
    key: &'a [u8],
    level: IndexLevel,
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
            let page_handle = self.tx.pager.fetch_page_read(page_id)?;
            println!("on page id {:?}", page_id);

            match page_handle.page_kind() {
                PageKind::IndexInternal => {
                    // We are still in internal and so must traverse down
                    page_id = page_handle.with_read(|page| {
                        let sp = SlottedPageRef::from_bytes(page);
                        let internal_page = IndexPageRef::from_slotted_page(sp);
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
        // TODO Finish
        todo!("finish")
    }
}

// Need to have insertpath structure for paths - which we can pass to a traverse_with_path?
// Need to have leafpos - basically slot entry for the leaf page this is only when we work inside the leaf page and should not be used outside of latch
// Need to have cursor/scan - for horizontal movement - scan cursor should logically sit between pages so splits will not interrupt scans and we can continue

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::buffer_manager::BufferManager;
    use crate::buffer::page_cache::{BaseFileCache, PageCache};
    use crate::buffer::page_frame::PageFrame;
    use crate::buffer::page_table::{NaiveMappingTable, PageTable};
    use crate::page::RawPage;
    use crate::page::SlottedPageMut;
    use crate::page::internal_page::{IndexCellOwned, IndexPageMut};
    use std::collections::VecDeque;
    use std::sync::Arc;

    struct TestTree {
        tree: OpCtx,
        pub stack: VecDeque<PageID>,
    }

    impl TestTree {
        fn tree(&self) -> BInner<'_> {
            BInner::new(&self.tree)
        }
        fn stack(&self) -> VecDeque<PageID> {
            self.stack.clone()
        }
    }

    fn three_level_tree<'a>() -> TestTree {
        // First setup the page manager
        // Add in a few pages with content
        // Setup OpCtx
        // Make a new tree
        // Traverse to find leaf
        let mut cache = Arc::new(BaseFileCache::new());
        let table = Arc::new(NaiveMappingTable::new());

        let mut stack = VecDeque::new();

        // Traversal will be
        // -- Page 123: Mercedes -> Child_ptr = 456, Volvo -> Child_ptr = 789
        // -- Left Page 456: Dodge -> Child_ptr = 987, Ford
        // Because we have a right child we can't fall off here and must have two keys in the root(above) layer
        // -- Right Page 789: Renault, Toyota
        // -- Left of Page 456 -- Page 987: Audi

        // Insert some pages into the cache

        let mut page1: RawPage = [0u8; 4096];
        let sp = SlottedPageMut::init_new(&mut page1, PageKind::IndexInternal.into());
        let mut internal1 = IndexPageMut::from_slotted_page(sp);

        internal1
            .add_cell_append_slot_entry(IndexCellOwned::new("Mercedes".as_bytes(), PageID(456)))
            .ok()
            .unwrap();
        internal1
            .add_cell_append_slot_entry(IndexCellOwned::new("Volvo".as_bytes(), PageID(789)))
            .ok()
            .unwrap();

        let mut page2: RawPage = [0u8; 4096];
        let sp = SlottedPageMut::init_new(&mut page2, PageKind::IndexInternal.into());
        let mut internal2 = IndexPageMut::from_slotted_page(sp);

        internal2
            .add_cell_append_slot_entry(IndexCellOwned::new("Dodge".as_bytes(), PageID(987)))
            .ok()
            .unwrap();
        internal2
            .add_cell_append_slot_entry(IndexCellOwned::new("Ford".as_bytes(), PageID(0)))
            .ok()
            .unwrap();

        let mut page3: RawPage = [0u8; 4096];
        let sp = SlottedPageMut::init_new(&mut page3, PageKind::IndexInternal.into());
        let mut internal3 = IndexPageMut::from_slotted_page(sp);

        internal3
            .add_cell_append_slot_entry(IndexCellOwned::new("Renault".as_bytes(), PageID(2)))
            .ok()
            .unwrap();
        internal3
            .add_cell_append_slot_entry(IndexCellOwned::new("Toyota".as_bytes(), PageID(1)))
            .ok()
            .unwrap();

        // Insert leaf we want to get
        let mut page4: RawPage = [0u8; 4096];
        let sp = SlottedPageMut::init_new(&mut page4, PageKind::IndexLeaf.into());
        let mut leaf1 = IndexPageMut::from_slotted_page(sp);

        leaf1
            .add_cell_append_slot_entry(IndexCellOwned::new("Audi".as_bytes(), PageID(987)))
            .ok()
            .unwrap();

        cache
            .insert(
                PageID(123),
                Arc::new(PageFrame::new(
                    PageID(123),
                    10,
                    PageKind::IndexInternal,
                    page1,
                )),
            )
            .ok()
            .unwrap();
        stack.push_back(PageID(123));
        cache
            .insert(
                PageID(456),
                Arc::new(PageFrame::new(
                    PageID(456),
                    10,
                    PageKind::IndexInternal,
                    page2,
                )),
            )
            .ok()
            .unwrap();
        stack.push_back(PageID(456));
        cache
            .insert(
                PageID(789),
                Arc::new(PageFrame::new(
                    PageID(789),
                    10,
                    PageKind::IndexInternal,
                    page3,
                )),
            )
            .ok()
            .unwrap();
        stack.push_back(PageID(789));
        cache
            .insert(
                PageID(987),
                Arc::new(PageFrame::new(PageID(987), 10, PageKind::IndexLeaf, page4)),
            )
            .ok()
            .unwrap();
        stack.push_back(PageID(987));

        let bm = BufferManager::new(cache, table);

        // Create a op ctx
        let op_ctx = OpCtx::new_fake_tx(10, Arc::new(bm));

        // Now we create a tree

        TestTree {
            tree: op_ctx,
            stack: stack,
        }
    }

    #[test]
    fn test_simple_traversal() {
        let tree_obj = three_level_tree();
        let tree = tree_obj.tree();
        let mut stack = tree_obj.stack();
        let root_page = stack.pop_front().unwrap();

        let result = tree.traverse_to_leaf(root_page, "Audi".as_bytes());
        match result {
            Ok(page_id) => {
                assert_eq!(page_id, stack.pop_back().unwrap());
                println!("Result = {:?}", page_id);
            }
            Err(err) => println!("Error = {:?}", err),
        }
    }
}
