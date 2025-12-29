//NOTE: This is the main intersection between disk and in-memory for pages

//NOTE: There are optimisations to be had where we look at all of the pages that we would need for the transaction
// and bitmask or something to get a set on the pages to ensure we are cache and have ready those pages and do not need
// to keep cycling them

//NOTE: We can also further optimise by

use crate::buffer::page_frame::PageFrame;
use crate::page::{PageID, PageKind};
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

pub(crate) trait PageCache: Send + Sync {
    fn fetch(&self, page_id: PageID) -> Result<Arc<PageFrame>>;
    fn insert(&self, page_id: PageID, frame: Arc<PageFrame>) -> Result<()>;
    fn remove(&self, page_id: PageID);
}

pub(crate) struct BaseFileCache {
    pub cache: Arc<Mutex<HashMap<PageID, Arc<PageFrame>>>>,
}

impl BaseFileCache {
    pub(crate) fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl PageCache for BaseFileCache {
    fn fetch(&self, page_id: PageID) -> Result<Arc<PageFrame>> {
        let cache = self.cache.lock().unwrap();

        if let Some(frame) = cache.get(&page_id) {
            Ok(frame.clone())
        } else {
            Err(PageCacheError::PageAllocationFailed)
        }
    }

    fn insert(&self, page_id: PageID, frame: Arc<PageFrame>) -> Result<()> {
        let mut cache = self.cache.lock().unwrap();
        let _ = cache.insert(page_id, frame);
        Ok(())
    }

    fn remove(&self, page_id: PageID) {
        let mut cache = self.cache.lock().unwrap();

        let _ = cache.remove(&page_id);
    }
}
