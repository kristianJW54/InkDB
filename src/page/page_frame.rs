use std::fmt::Error;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicBool;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use crate::page::{PageID, PageKind, RawPage, SlotID};
use crate::page::raw_page::{SlottedPage};
// NOTE: Above PageFrame would be something like the b-tree node/tree


pub (crate) struct PageFrame {
    id: PageID,
    page_type: PageKind,
    //txid?
    dirty: AtomicBool,
    inner_page: RwLock<SlottedPage>,
    // more meta data
}

impl PageFrame {

    pub(crate) fn new_frame_from_page(id: PageID, page_type: PageKind, page: SlottedPage) -> Self {
        Self { id, page_type, dirty: AtomicBool::new(false), inner_page: RwLock::new(page), }
    }

    pub(crate) fn page_read_guard<'page>(&self) -> PageReadGuard<'_> {
        PageReadGuard { latch_read: self.inner_page.read().unwrap() }
    }

    pub(crate) fn page_write_guard<'page>(&self) -> PageWriteGuard<'_> {
        PageWriteGuard { latch_write: self.inner_page.write().unwrap() }
    }

    pub(crate) fn page_type(&self) -> PageKind {
        self.page_type
    }


}

#[derive(Debug)]
pub(crate) struct PageReadGuard<'a> {
    latch_read: RwLockReadGuard<'a, SlottedPage>,
}

impl <'a> Deref for PageReadGuard<'a> {
    type Target = SlottedPage;
    fn deref(&self) -> &Self::Target {
        &*self.latch_read
    }
}

pub(crate) struct PageWriteGuard<'a> {
    latch_write: RwLockWriteGuard<'a, SlottedPage>
}

impl <'a> Deref for PageWriteGuard<'a> {
    type Target = SlottedPage;
    fn deref(&self) -> &Self::Target { &*self.latch_write }
}

impl <'a> DerefMut for PageWriteGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut *self.latch_write }
}


