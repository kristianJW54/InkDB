//NOTE: This is the main intersection between disk and in-memory for pages

//NOTE: There are optimisations to be had where we look at all of the pages that we would need for the transaction
// and bitmask or something to get a set on the pages to ensure we are cache and have ready those pages and do not need
// to keep cycling them

//NOTE: We can also further optimise by

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(crate) type Result<T> = std::result::Result<T, PageCacheError>;

#[derive(Debug)]
pub(crate) enum PageCacheError {
    PageAllocationFailed,
}

// Cache owns the lock/access caller owns the result
//
// We pass to the closure the raw bytes meaning that only the cache interacts with the page frame and manages the locks, dirty, flags etc
// it is then the responsibility of the caller to interpret the bytes and use them accordingly
// FnMut() is used here as it allows mutability within the scope of the closure NOT on the bytes itself which are under their respective lock from the cache

pub trait PageCache {
    fn get(self, page_id: PageID, f: &mut dyn FnMut(&[u8]));
    fn put(&self, page: PageFrame) -> Result<()>;
    fn remove(&self, page_id: PageID) -> Result<()>;
    fn allocate(&self, page_id: PageID, kind: PageKind) -> Result<PageHandle>;
}

pub struct BaseFileCache {
    pub cache: Mutex<HashMap<PageID, Arc<PageFrame>>>,
    // Need a transaction table
    // Need a free list
    // Need a next pageID
    // A next TransactionID?
}

impl BaseFileCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }
}
