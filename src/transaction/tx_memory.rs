use std::sync::Arc;
use crate::page_cache::base_file_cache::BaseFileCache;

pub(crate) struct TxMemory {
    pub cache: Arc<BaseFileCache>,
    // allocator
    // id
    // snapshot?
}

// NOTE: On page creation we embed the max transaction id into the page