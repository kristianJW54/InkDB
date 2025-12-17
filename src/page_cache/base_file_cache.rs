//NOTE: This is the main intersection between disk and in-memory for pages

//NOTE: There are optimisations to be had where we look at all of the pages that we would need for the transaction
// and bitmask or something to get a set on the pages to ensure we are cache and have ready those pages and do not need
// to keep cycling them

//NOTE: We can also further optimise by

use crate::page::{PageID, PageKind};
use crate::page_cache::page_frame::{PageFrame, PageReadGuard, PageWriteGuard};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(crate) type Result<T> = std::result::Result<T, PageCacheError>;

#[derive(Debug)]
pub(crate) enum PageCacheError {
    PageAllocationFailed,
}

#[derive(Debug)]
pub(crate) struct PageHandle {
    page: Arc<PageFrame>,
}

impl PageHandle {
    pub fn new(page: Arc<PageFrame>) -> Self {
        Self { page }
    }

    pub fn read(&self) -> PageReadGuard<'_> {
        self.page.page_read_guard()
    }

    pub fn write(&mut self) -> PageWriteGuard<'_> {
        self.page.page_write_guard()
    }
}

pub trait PageCache {
    fn get(&self, page_id: PageID) -> Option<PageHandle>;
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

impl PageCache for BaseFileCache {
    fn get(&self, page_id: PageID) -> Option<PageHandle> {
        self.cache
            .lock()
            .unwrap()
            .get(&page_id)
            .map(|page| PageHandle { page: page.clone() })
    }

    fn put(&self, page: PageFrame) -> Result<()> {
        self.cache
            .lock()
            .unwrap()
            .insert(page.page_id(), Arc::new(page));
        Ok(())
    }

    fn remove(&self, page_id: PageID) -> Result<()> {
        self.cache.lock().unwrap().remove(&page_id);
        Ok(())
    }

    fn allocate(&self, page_id: PageID, kind: PageKind) -> Result<PageHandle> {
        let page = PageFrame::new(page_id, kind);
        self.put(page)?;
        self.get(page_id)
            .ok_or(PageCacheError::PageAllocationFailed)
    }
}
