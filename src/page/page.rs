

// The main page frame which is stored in the page_cache

use std::cell::{UnsafeCell};
use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;
use crate::page::base_page::{InnerPage, RawPage};
use crate::page::{PageID, PageType};

// The Page will be shared and referenced by many different things (transaction, cache, reader, writer)
pub(crate) struct Page {
    id: PageID,
    // We need to be able to get a mutable view on page despite having many shared references active
    page_inner: InnerPage,
    // We store the latch separate to quickly capture what we need and then perform operations outside the latch
    // which we know don't conflict due to MVCC
    latch: RwLock<()>,
    dirty: AtomicBool,
}

impl Page {
    pub fn new(id: PageID, bytes: RawPage) -> Self {
        Self { id, page_inner: InnerPage::new(), latch: RwLock::new(()), dirty: AtomicBool::new(false) }
    }

    pub fn write(&self, bytes: &mut [u8]) -> Result<(), ()> {

        assert!(bytes.len() <= 1000);

        Ok(())
    }

    pub fn print_data(&self) {
        self.page_inner.print_data();
    }
}


// TODO Test here

#[test]
fn test_page_print() {

    let page = Page::new(PageID(2), RawPage::new());



}