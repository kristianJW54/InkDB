use std::collections::{HashMap, VecDeque};
use std::io::{Error, ErrorKind};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use crate::page::page_frame::PageFrame;
use crate::page::{PageID, PageType};
use crate::page::index_page::{IndexPageRef};
use crate::page::PageType::Index;
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

    //NOTE: This should be replaced by the get_page() method on the cache
    pub fn get_page(&self) -> Result<Arc<PageFrame>, Error> {
        let cache = self.txm.cache.deref();
        let map = cache.cache.lock().unwrap();

        //NOTE: We don't care if the cache has the page - it is the job of the cache to fetch from disk if it's not cached
        let value = map.get(&PageID(1));
        // Do something with the value?

        if value.is_some() {
            return Ok(value.unwrap().clone());
        } else {
            Err(Error::new(ErrorKind::Other, "page not found"))
        }
    }

    // TODO Build descend methods - think about different things we would need to do in there like siblings, splits, etc and also think about page semantics

    fn descend_level(&self, key: &[u8]) -> Result<Option<Arc<PageFrame>>, Error> {

        // At the moment I'm not using page specific type? Do we want to here?

        let current = self.current.clone(); // We are not cloning the page, we are just creating another Arc<Page> ref
        // I think we wrap current in the

        match current.page_type() {
            PageType::Index => {
                let guard = current.page_read_guard();
                let index_page = IndexPageRef::from_guard(guard);
                // Need to match on the index page type

            },
            _ => {},
        }


        Err(Error::new(ErrorKind::Other, "level not found"))
    }

    // NOTE: What type of storing do we want? Data in cells? Index table with pointer (Indirection)

}

