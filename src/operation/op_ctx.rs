use crate::buffer::buffer_manager::PageManager;
use std::sync::Arc;

pub(crate) struct OpCtx {
    pub pager: Arc<(dyn PageManager)>,
    // allocator
    pub id: u64,
    // snapshot?
}

// NOTE: On page creation we embed the max transaction id into the page

impl OpCtx {
    pub fn new_fake_tx(id: u64, pager: Arc<(dyn PageManager)>) -> Self {
        Self { pager, id }
    }
}
