use crate::index::index_page::{IndexPageError, IndexPageRef};
use crate::page::{PageID, PageKind};
use crate::page_cache::page_frame::PageFrame;
use crate::transaction::tx_memory::TxMemory;
use std::collections::{HashMap, VecDeque};
use std::ops::Deref;
use std::sync::Arc;

// Base tree error
pub(crate) type Result<T> = std::result::Result<T, BaseTreeError>;

#[derive(Debug)]
pub(crate) enum BaseTreeError {
    // We could wrap a lower level error type by saying Page(SlottedPageErr)
    IndexPageError(IndexPageError),
    DescentError { level: usize, error: &'static str },
    PageNotFound,
}

impl From<IndexPageError> for BaseTreeError {
    fn from(error: IndexPageError) -> Self {
        BaseTreeError::IndexPageError(error)
    }
}

// Btree base structure and heavy lifting

// We start with a cursor which is our main vehicle in the tree

pub(crate) struct Cursor<'blink> {
    txm: &'blink TxMemory,
    stack: VecDeque<Arc<PageFrame>>,
    current: Arc<PageFrame>,
    // Any slot specific fields?
}

impl<'blink> Cursor<'blink> {
    pub(super) fn new(txm: &'blink TxMemory, starting_frame: Arc<PageFrame>) -> Self {
        let vec = VecDeque::new();
        Self {
            txm,
            stack: vec,
            current: starting_frame,
        }
    }

    //NOTE: This should be replaced by the get_page() method on the cache
    pub fn get_page(&self) -> Result<Arc<PageFrame>> {
        let cache = self.txm.cache.deref();
        let map = cache.cache.lock().unwrap();

        //NOTE: We don't care if the cache has the page - it is the job of the cache to fetch from disk if it's not cached
        let value = map.get(&PageID(1));
        // Do something with the value?

        if value.is_some() {
            return Ok(value.unwrap().clone());
        } else {
            Err(BaseTreeError::PageNotFound)
        }
    }

    // TODO Build descend methods - think about different things we would need to do in there like siblings, splits, etc and also think about page semantics

    fn descend_level(&self, key: &[u8]) -> Result<Option<PageID>> {
        let current = self.current.clone(); // We are not cloning the page, we are just creating another Arc<Page> ref

        match current.page_type() {
            PageKind::Index => {
                let guard = current.page_read_guard();
                let index_page = IndexPageRef::from_guard(guard);
                // Need to match on the index page level
                return match index_page.level().into() {
                    0 => Err(BaseTreeError::DescentError {
                        level: 0,
                        error: "we are a leaf",
                    }),
                    _ => {
                        let child_ptr = index_page.find_child_ptr(key)?;
                        Ok(child_ptr)
                    }
                };
            }
            _ => Err(BaseTreeError::DescentError {
                level: 0,
                error: "found nothing?",
            }),
        }
    }
}

// NOTE: What type of storing do we want? Data in cells? Index table with pointer (Indirection)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_page::{IndexCellOwned, IndexLevel, IndexPageOwned};
    use crate::page_cache::base_file_cache::BaseFileCache;

    #[test]
    fn test_basic_descend_level() {
        // First create two pages
        // Level zero (leaf) and Internal

        let mut level_zero_page = IndexPageOwned::new(0);
        level_zero_page.set_level(IndexLevel(0));
        //
        let mut level_one_page = IndexPageOwned::new(0);
        level_one_page.set_level(IndexLevel(1));

        // We add some cells so we can descend on logic
        // We add cell "Ford" with child_ptr of PageID(1234 as u64) any key which is less than "Ford" alphabetically will be given the child_ptr
        // We then add cell "Jaguar" with child_ptr of PageID(3456 as u64) any key which is less then "Jaguar" and greater than "Ford"
        // alphabetically will be given the child_ptr
        level_one_page
            .add_cell_append_slot_entry(IndexCellOwned::new("Ford".as_bytes(), PageID(1234 as u64)))
            .unwrap();
        level_one_page
            .add_cell_append_slot_entry(IndexCellOwned::new(
                "Jaguar".as_bytes(),
                PageID(3456 as u64),
            ))
            .unwrap();

        // Now we create frames for the pages

        let level_zero_frame = PageFrame::new_frame_from_page(
            PageID(1234 as u64),
            level_zero_page.kind(),
            level_zero_page.into_inner(),
        );

        let level_one_frame = PageFrame::new_frame_from_page(
            PageID(3456 as u64),
            level_one_page.kind(),
            level_one_page.into_inner(),
        );

        // If we try to descend on level zero we should get an 'error' message

        let txm = TxMemory::new_fake_tx(0, Arc::new(BaseFileCache::new()));

        txm.cache
            .cache
            .lock()
            .unwrap()
            .insert(PageID(1234 as u64), Arc::new(level_zero_frame));

        let cursor = Cursor::new(&txm, Arc::new(level_one_frame));

        let result = cursor.descend_level("Ford".as_bytes());
        match result {
            Ok(Some(page)) => {
                // I should have the PageID(0) here and should be able to fetch this from the cahce

                assert_eq!(page, PageID(3456 as u64));
            }
            Ok(None) => {}
            Err(e) => {
                println!("{:?}", e);
            }
        }
    }
}
