use std::sync::Arc;
use crate::page::page::PageFrame;
use crate::page::PageID;
use crate::transaction::tx_memory::TxMemory;
// Btree base structure and heavy lifting

// We start with a cursor which is our main vehicle in the tree

pub(crate) struct Cursor<'blink> {
    txm: &'blink TxMemory,
    stack: Vec<Arc<PageFrame>>,
    current: Arc<PageFrame>,
    // Any slot specific fields?
}

