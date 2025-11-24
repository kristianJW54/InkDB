use std::sync::Arc;
use crate::page_cache::base_file_cache::BaseFileCache;

pub(crate) struct TxMemory {
    pub cache: Arc<BaseFileCache>,
    // allocator
    pub id: u64,
    // snapshot?
}

// NOTE: On page creation we embed the max transaction id into the page

impl TxMemory {
    pub fn new_fake_tx(id: u64, cache: Arc<BaseFileCache>) -> Self {
        Self { cache,  id, }
    }
}