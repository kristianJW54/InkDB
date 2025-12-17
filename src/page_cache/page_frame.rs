use crate::page::SlottedPage;
use crate::page::{PageID, PageKind, RawPage, SlotID};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

// NOTE: Above PageFrame would be something like the b-tree node/tree

#[derive(Debug)]
pub(super) struct PageFrame {
    id: PageID,
    page_type: PageKind,
    //txid?
    dirty: AtomicBool,
    inner_page: RwLock<SlottedPage>,
    pin_count: AtomicUsize,
    // more meta data
}

impl PageFrame {
    pub(super) fn new(id: PageID, page_type: PageKind) -> Self {
        let page = SlottedPage::new_blank();
        Self {
            id,
            page_type,
            dirty: AtomicBool::new(false),
            inner_page: RwLock::new(page),
            pin_count: AtomicUsize::new(0),
        }
    }

    pub(crate) fn page_id(&self) -> PageID {
        self.id
    }

    pub(super) fn page_read_guard<'page>(&self) -> PageReadGuard<'_> {
        PageReadGuard {
            latch_read: self.inner_page.read().unwrap(),
        }
    }

    pub(super) fn page_write_guard<'page>(&self) -> PageWriteGuard<'_> {
        PageWriteGuard {
            latch_write: self.inner_page.write().unwrap(),
        }
    }

    pub(super) fn page_type(&self) -> PageKind {
        self.page_type
    }
}

#[derive(Debug)]
pub(crate) struct PageReadGuard<'a> {
    latch_read: RwLockReadGuard<'a, SlottedPage>,
}

impl<'a> Deref for PageReadGuard<'a> {
    type Target = SlottedPage;
    fn deref(&self) -> &Self::Target {
        &*self.latch_read
    }
}

pub(crate) struct PageWriteGuard<'a> {
    latch_write: RwLockWriteGuard<'a, SlottedPage>,
}

impl<'a> Deref for PageWriteGuard<'a> {
    type Target = SlottedPage;
    fn deref(&self) -> &Self::Target {
        &*self.latch_write
    }
}

impl<'a> DerefMut for PageWriteGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.latch_write
    }
}
