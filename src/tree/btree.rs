use crate::buffer::page_cache::PageHandle;
use crate::page::PageID;
use crate::transaction::tx_memory::TxMemory;
use std::sync::Arc;

//--------------------

pub(crate) struct Blink {
    pub(super) id: PageID,
    pub(super) tx_mem: TxMemory, // NOTE: Should be transaction structure with arc cache within
    pub(super) meta_page: PageHandle,
}
