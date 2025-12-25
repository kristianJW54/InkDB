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

    pub(crate) fn slotted_ref(&self) -> Result<SlottedPageRef<'_>> {
        if self.kind.uses_slotted_page_layout() {
            Ok(SlottedPageRef::from_bytes(self.raw()))
        } else {
            Err(PageFrameError::InvalidPageKind)
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::SlottedPageMut;
    use crate::page::internal_page::IndexPageRef;

    #[test]
    fn get_internal_index_page() {
        let mut raw_page: RawPage = [0u8; 4096];
        let sp = SlottedPageMut::init_new(&mut raw_page);
        // --------------------------
        // TODO Fix the API here from slotted page to page interpreted layer
        // --------------------------
        let index_internal = IndexPageRef::from_slotted_page(sp);

        let frame = PageFrame::new(10, PageKind::IndexInternal, raw_page);

        // We take a read only view of the page inside the frame

        {
            let ref_guard = frame.read_guard();
            let sp_ref = ref_guard.slotted_ref().ok().unwrap();
        }
    }
}
