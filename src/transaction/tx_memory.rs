use crate::page_cache::base_file_cache::{BaseFileCache, PageCache};
use std::sync::Arc;

pub(crate) struct TxMemory {
    pub cache: Arc<dyn PageCache>,
    // allocator
    pub id: u64,
    // snapshot?
}

// NOTE: On page creation we embed the max transaction id into the page

impl TxMemory {
    pub fn new_fake_tx(id: u64, cache: Arc<dyn PageCache>) -> Self {
        Self { cache, id }
    }
}
