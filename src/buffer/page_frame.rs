use crate::page::{PageKind, RawPage};
use crate::page::{SlottedPageMut, SlottedPageRef};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU16};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub(super) type Result<T> = std::result::Result<T, PageFrameError>;

pub(super) enum PageFrameError {
    InvalidPageKind,
}

pub(crate) struct PageFrame {
    checksum: u32,
    kind: PageKind,
    dirty: AtomicBool,
    latch: RwLock<RawPage>,
    pin: AtomicU16,
}

impl PageFrame {
    pub(crate) fn new(checksum: u32, kind: PageKind, raw_page: RawPage) -> Self {
        Self {
            checksum,
            kind,
            dirty: AtomicBool::new(false),
            latch: RwLock::new(raw_page),
            pin: AtomicU16::new(0),
        }
    }

    pub(crate) fn read_guard(&self) -> FrameReadGuard {
        FrameReadGuard::new(self.latch.read().unwrap(), self.kind.clone())
    }

    pub(crate) fn write_guard(&self) -> FrameWriteGuard {
        FrameWriteGuard::new(self.latch.write().unwrap(), self.kind.clone())
    }
}

// Need read and write guards to return slotted page views

pub(super) struct FrameReadGuard<'a> {
    page: RwLockReadGuard<'a, RawPage>,
    kind: PageKind,
}

impl<'a> FrameReadGuard<'a> {
    fn new(page: RwLockReadGuard<'a, RawPage>, kind: PageKind) -> Self {
        Self { page, kind }
    }

    fn raw(&self) -> &RawPage {
        &self.page
    }
}

impl<'a> Deref for FrameReadGuard<'a> {
    type Target = RawPage;

    fn deref(&self) -> &Self::Target {
        &self.page
    }
}

pub(super) struct FrameWriteGuard<'a> {
    page: RwLockWriteGuard<'a, RawPage>,
    kind: PageKind,
}

impl<'a> FrameWriteGuard<'a> {
    fn new(page: RwLockWriteGuard<'a, RawPage>, kind: PageKind) -> Self {
        Self { page, kind }
    }

    fn raw(&mut self) -> &mut RawPage {
        &mut self.page
    }

    pub(crate) fn slotted_mut(&mut self) -> Result<SlottedPageMut<'_>> {
        if self.kind.uses_slotted_page_layout() {
            Ok(SlottedPageMut::from_bytes(&mut self.page))
        } else {
            Err(PageFrameError::InvalidPageKind)
        }
    }
}

impl<'a> Deref for FrameWriteGuard<'a> {
    type Target = RawPage;

    fn deref(&self) -> &Self::Target {
        &self.page
    }
}

impl<'a> DerefMut for FrameWriteGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.page
    }
}
