use crate::page::PageID;
use crate::page_cache::base_file_cache::PageHandle;
use crate::transaction::tx_memory::TxMemory;
use std::sync::Arc;

//--------------------

pub(crate) struct Blink {
    pub(super) id: PageID,
    pub(super) tx_mem: TxMemory, // NOTE: Should be transaction structure with arc cache within
    pub(super) meta_page: PageHandle,
}
