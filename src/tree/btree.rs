use std::sync::Arc;
use crate::page::PageID;
use crate::page_cache::base_file_cache::BaseFileCache;
use crate::transaction::tx_memory::TxMemory;

pub(crate) struct Blink {
    id: PageID,
    tx_mem: TxMemory, // NOTE: Should be transaction structure with arc cache within
}

