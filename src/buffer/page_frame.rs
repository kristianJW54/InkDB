use crate::page::internal_page::InternalPageError;
use crate::page::{PageID, PageKind, RawPage};
use crate::page::{SlottedPageMut, SlottedPageRef};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub(crate) type Result<T> = std::result::Result<T, PageFrameError>;

#[derive(Debug)]
pub(crate) enum PageFrameError {
    IndexPageError(InternalPageError),
    InvalidPageKind,
}

impl From<InternalPageError> for PageFrameError {
    fn from(err: InternalPageError) -> Self {
        PageFrameError::IndexPageError(err)
    }
}

pub(crate) struct PageFrame {
    id: PageID,
    checksum: u32,
    kind: PageKind,
    dirty: AtomicBool,
    latch: RwLock<RawPage>,
    pin: AtomicU16,
}

impl PageFrame {
    pub(crate) fn new(id: PageID, checksum: u32, kind: PageKind, raw_page: RawPage) -> Self {
        Self {
            id,
            checksum,
            kind,
            dirty: AtomicBool::new(false),
            latch: RwLock::new(raw_page),
            pin: AtomicU16::new(0),
        }
    }

    pub(crate) fn page_kind(&self) -> PageKind {
        self.kind
    }

    pub(super) fn read_guard<'a>(&'a self) -> FrameReadGuard<'a> {
        FrameReadGuard::new(self.latch.read().unwrap(), self.kind.clone())
    }

    pub(super) fn write_guard<'a>(&'a self) -> FrameWriteGuard<'a> {
        FrameWriteGuard::new(self.latch.write().unwrap(), self.kind.clone())
    }

    /// Executes `f` while holding a read latch on the page.
    /// The closure may return any error `E` convertible into `PageFrameError`.
    /// No references to page memory may escape the closure.
    pub(crate) fn with_read<F, T, E>(&self, f: F) -> Result<T>
    where
        // We require the closure to return any error E that can be converted into PageFrameError - not the aliased Result<T>
        F: FnOnce(&RawPage) -> std::result::Result<T, E>,
        E: Into<PageFrameError>,
    {
        let frame = self.read_guard();
        f(frame.raw()).map_err(Into::into)
    }

    pub(crate) fn with_write<F, T, E>(&self, f: F) -> Result<T>
    where
        // We require the closure to return any error E that can be converted into PageFrameError - not the aliased Result<T>
        F: FnOnce(&mut RawPage) -> std::result::Result<T, E>,
        E: Into<PageFrameError>,
    {
        let mut frame = self.write_guard();
        f(frame.raw()).map_err(Into::into)
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

    pub(super) fn slotted_ref(&self) -> Result<SlottedPageRef<'_>> {
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

    pub(super) fn slotted_mut(&mut self) -> Result<SlottedPageMut<'_>> {
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
    use crate::page::internal_page::{InternalPageMut, InternalPageRef};

    #[test]
    fn get_internal_index_page() {
        let mut raw_page: RawPage = [0u8; 4096];
        let sp = SlottedPageMut::init_new(&mut raw_page, PageKind::Undefined.into());
        let mut index_internal = InternalPageMut::from_slotted_page(sp);

        index_internal.set_page_type(PageKind::IndexInternal);
        println!("Internal kind = {:?}", index_internal.kind());
        let frame = PageFrame::new(PageID(1), 10, PageKind::IndexInternal, raw_page);

        // We take a read only view of the page inside the frame

        frame
            .with_read(|rp| {
                let ref_guard = InternalPageRef::from_slotted_page(SlottedPageRef::from_bytes(rp));
                println!("Page Kind {:?}", ref_guard.kind());
                Ok::<(), PageFrameError>(())
            })
            .ok()
            .unwrap();
    }
}
