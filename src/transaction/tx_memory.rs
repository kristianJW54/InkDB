use std::sync::Arc;
use crate::page_cache::base_file_cache::BaseFileCache;

pub(crate) struct TxMemory {
    cache: Arc<BaseFileCache>,
}