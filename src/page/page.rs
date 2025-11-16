

// The main page frame which is stored in the page_cache

use std::cell::{UnsafeCell};
use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;
use crate::page::base_page::RawPage;
use crate::page::{PageID, PageType};

// The Page will be shared and referenced by many different things (transaction, cache, reader, writer)
pub(crate) struct Page {
    id: PageID,
    // We need to be able to get a mutable view on page despite having many shared references active
    page_inner: UnsafeCell<RawPage>,
    // We store the latch separate to quickly capture what we need and then perform operations outside the latch
    // which we know don't conflict due to MVCC
    latch: RwLock<()>,
    dirty: AtomicBool,
}

impl Page {
    pub fn new(id: PageID, bytes: RawPage) -> Self {
        Self { id, page_inner: UnsafeCell::new(bytes), latch: RwLock::new(()), dirty: AtomicBool::new(false) }
    }

    pub fn write(&mut self, bytes: &mut [u8]) -> Result<(), ()> {

        assert!(bytes.len() <= 1000);

        // We need to lock and then we can get_mut page and then write

        let guard = self.latch.write().unwrap();
        let page_data = self.page_inner.get_mut();
        page_data.write(bytes).map_err(|_| ())?;
        Ok(())
    }
}