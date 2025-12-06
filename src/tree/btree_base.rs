use std::collections::{HashMap, VecDeque};
use std::io::{Error, ErrorKind};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use crate::page::page_frame::PageFrame;
use crate::page::{PageID, PageKind};
use crate::page::index_page::{IndexPageRef};
use crate::page::PageKind::Index;
use crate::transaction::tx_memory::TxMemory;

// Base tree error
type Result<T> = std::result::Result<T, BaseTreeError>;

#[derive(Debug)]
enum BaseTreeError {
    // We could wrap a lower level error type by saying Page(SlottedPageErr)
    DescentError{
        level: usize,
        error: &'static str,
    },
    PageNotFound
}

// Btree base structure and heavy lifting

// We start with a cursor which is our main vehicle in the tree

pub(crate) struct Cursor<'blink> {
    txm: &'blink TxMemory,
    stack: VecDeque<Arc<PageFrame>>,
    current: Arc<PageFrame>,
    // Any slot specific fields?
}

impl <'blink> Cursor<'blink> {

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

    fn descend_level(&self, key: &[u8]) -> Result<Option<Arc<PageFrame>>> {

        // At the moment I'm not using page specific type? Do we want to here?

        let current = self.current.clone(); // We are not cloning the page, we are just creating another Arc<Page> ref
        // I think we wrap current in the

        match current.page_type() {
            PageKind::Index => {
                let guard = current.page_read_guard();
                let index_page = IndexPageRef::from_guard(guard);
                // Need to match on the index page level
                return match index_page.level().into() {
                    0 => {
                        println!("we are a leaf - can't descend no more");
                        Err(BaseTreeError::DescentError { level: 0, error: "we are a leaf" })
                    }
                    _ => Err(BaseTreeError::DescentError { level: index_page.level().into() as usize, error: "supposed to have found a leaf" })
                }

            },
            _ => Err(BaseTreeError::DescentError { level: 0, error: "found nothing?" })}
        }
    }

    // NOTE: What type of storing do we want? Data in cells? Index table with pointer (Indirection)



#[cfg(test)]
mod tests {
    use crate::page::index_page::{IndexLevel, IndexPageOwned};
    use crate::page_cache::base_file_cache::BaseFileCache;
    use super::*;

    #[test]
    fn test_basic_descend_level() {

        let mut level_zero_page = IndexPageOwned::new(0);
        level_zero_page.set_level(IndexLevel(0));
        //
        let mut level_one_page = IndexPageOwned::new(0);
        level_one_page.set_level(IndexLevel(1));

        // If we try to descend on level zero we should get an 'error' message

        let txm = TxMemory::new_fake_tx(0, Arc::new(BaseFileCache::new()));

        let cursor = Cursor::new(
            &txm,
            Arc::new(PageFrame::new_frame_from_page(PageID(0), level_zero_page.kind(), level_zero_page.into_inner())));

        let result = cursor.descend_level(&[0u8; 4]);
        match result {
            Ok(Some(page)) => {},
            Ok(None) => {},
            Err(e) => {
                println!("{:?}", e);
            },
        }
    }
}