use std::sync::Arc;

pub(crate) struct TxMemory {
    pub cache: Arc<()>,
    // allocator
    pub id: u64,
    // snapshot?
}

// NOTE: On page creation we embed the max transaction id into the page

impl TxMemory {
    pub fn new_fake_tx(id: u64, cache: Arc<()>) -> Self {
        Self { cache, id }
    }
}
