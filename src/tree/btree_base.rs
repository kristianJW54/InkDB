use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use crate::page::page::PageFrame;
use crate::page::PageID;
use crate::transaction::tx_memory::TxMemory;
// Btree base structure and heavy lifting

// We start with a cursor which is our main vehicle in the tree

pub(crate) struct Cursor<'blink> {
    txm: &'blink TxMemory,
    stack: VecDeque<Arc<PageFrame>>,
    current: Arc<PageFrame>,
    // Any slot specific fields?
}

impl Cursor<'_> {

    pub fn get_page(&self) -> Option<Arc<PageFrame>> {
        let cache = self.txm.cache.deref();
        let map = cache.cache.lock().unwrap();
        let value = map.get(&PageID(1));
        // Do something with the value?

        if value.is_some() {
            return Some(value.unwrap().clone());
        } else {
            None
        }
    }

    // TODO Build descend methods - think about different things we would need to do in there like siblings, splits, etc and also think about page semantics

}

