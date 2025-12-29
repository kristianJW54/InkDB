use crate::buffer::page_cache::PageCache;
use crate::buffer::page_frame::{FrameReadGuard, FrameWriteGuard, PageFrame};
use crate::buffer::page_table::PageTable;
use crate::page::PageID;
use std::sync::Arc;

// Need a transaction table
// Need a free list
// Need a next pageID
// A next TransactionID?
//

pub(crate) type Result<T> = std::result::Result<T, BufferManagerError>;

#[derive(Debug)]
pub(crate) enum BufferManagerError {
    CacheError,
    TableError,
    ManagerError,
}

pub(crate) trait PageManager: Send + Sync {
    fn fetch_page_read(&self, page_id: PageID) -> Result<Arc<PageFrame>>;
}

pub(crate) struct BufferManager {
    cache: Arc<dyn PageCache>,
    table: Arc<dyn PageTable>,
}

impl BufferManager {
    pub(crate) fn new(cache: Arc<dyn PageCache>, table: Arc<dyn PageTable>) -> Self {
        Self { cache, table }
    }
}

impl PageManager for BufferManager {
    fn fetch_page_read(&self, page_id: PageID) -> Result<Arc<PageFrame>> {
        if let Ok(frame) = self.cache.fetch(page_id) {
            Ok(frame)
        } else {
            Err(BufferManagerError::CacheError)
        }
    }
}
