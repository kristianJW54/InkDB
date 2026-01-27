use crate::page::{
    PAGE_SIZE, PageFlags, PageStates, read_u64_le_unsafe,
    slotted_page::{HEADER_SIZE_U16, PREFIX_SIZE, RIGHT_SIBLING_OFFSET, TRAILER_OFFSET},
    write_u64_le_unsafe,
};

use super::{PageID, PageKind, PageType, SlottedPageMut, SlottedPageRef};

pub(super) enum IndexPageError {
    //
}

/// IndexPageRef holds the SlottedPageRef which is under the lifetime of the guard, given out by the PageFrame
/// After we perform operations on a IndexPage, the IndexPageRef is dropped and SlottedPageRef is returned back
/// to the guard.
/// IndexPageRef defines read only methods for indexing logic over the Slotted Page layout.
pub(super) struct IndexPageRef<'page> {
    bytes: SlottedPageRef<'page>,
}

impl<'page> IndexPageRef<'page> {
    pub(super) fn from_slotted_page(page: SlottedPageRef<'page>) -> Self {
        Self { bytes: page }
    }

    pub(super) fn get_page_type(&self) -> PageType {
        PageType::from(self.bytes.get_page_type())
    }

    pub(super) fn kind(&self) -> PageKind {
        self.get_page_type().page_kind()
    }

    pub(super) fn flags(&self) -> PageFlags {
        PageFlags::from(self.bytes.get_flags())
    }

    pub(super) fn page_sub_type(&self) -> u8 {
        self.get_page_type().page_sub_type()
    }

    pub(super) fn get_left_sibling(&self) -> Option<PageID> {
        // SAFETY: The pointer is within the bounds of the page bytes and is aligned for reading a u64.
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(TRAILER_OFFSET + PREFIX_SIZE);
            let id = PageID::from(read_u64_le_unsafe(b_ptr));
            if id.0 == 0 { None } else { Some(id) }
        }
    }

    pub(super) fn get_right_sibling(&self) -> Option<PageID> {
        // SAFETY: The pointer is within the bounds of the page bytes and is aligned for reading a u64.
        unsafe {
            let b_ptr = self
                .bytes
                .as_ptr()
                .add(TRAILER_OFFSET + PREFIX_SIZE + RIGHT_SIBLING_OFFSET);
            let id = PageID::from(read_u64_le_unsafe(b_ptr));
            if id.0 == 0 { None } else { Some(id) }
        }
    }

    // TODO: Need to implement the get prefix cells and cell handling - think about how we want to interact with IndexCell
}

/// IndexPageMut holds the SlottedPageMut which is under the lifetime of the guard, given out by the PageFrame
/// After we perform operations on a page, IndexPage wrappers are dropped and SlottedPage is returned back
/// to the guard
/// IndexPageMut provides mutable methods for indexing logic and operations over the Slotted Page layout
pub(super) struct IndexPageMut<'page> {
    bytes: SlottedPageMut<'page>,
}

impl<'page> IndexPageMut<'page> {
    pub(super) fn from_slotted_page(
        /* Should we re-borrow mut here? */ page: SlottedPageMut<'page>,
    ) -> Self {
        Self { bytes: page }
    }

    pub(super) fn as_ref<'a>(&'a self) -> IndexPageRef<'a> {
        IndexPageRef::from_slotted_page(self.bytes.as_ref())
    }

    pub(super) fn init_in_place(&mut self, lsn: u64, page_type: PageType, flags: PageFlags) {
        self.bytes.wipe_page();
        self.bytes.set_page_type(page_type.into());
        // Set free start to default HEADER_SIZE
        self.bytes.set_free_start(HEADER_SIZE_U16);
        // Set lsn
        self.bytes.set_lsn(lsn);
        // Set flags
        self.bytes.set_flags(flags.0);
        // We don't need to do anything else here as trailer space is concrete and slot array should be empty
        // We have type and flags which should be sufficient for correctness
    }

    pub(super) fn set_page_type(&mut self, page_type: PageType) {
        self.bytes.set_page_type(page_type.into());
    }

    pub(super) fn get_page_type(&self) -> PageType {
        PageType::from(self.bytes.get_page_type())
    }

    pub(super) fn set_sub_type(&mut self, sub_type: u8) {
        let mut new_pt = self.get_page_type();
        new_pt.set_subtype_page_bits(sub_type);
        self.set_page_type(new_pt.into());
    }

    pub(super) fn set_left_sibling(&mut self, left_sibling: PageID) {
        // SAFETY: We have exclusive access to the slotted page and Rawpage bytes
        // We know the offset to the left sibling in the page which is fixed so we are safe to write to it
        unsafe {
            let b_ptr = self.bytes.as_mut_ptr().add(TRAILER_OFFSET + PREFIX_SIZE);
            write_u64_le_unsafe(b_ptr, left_sibling.into());
        }
    }

    pub(super) fn set_right_sibling(&mut self, right_sibling: PageID) {
        // SAFETY: We have exclusive access to the slotted page and Rawpage bytes
        // We know the offset to the right sibling in the page which is fixed so we are safe to write to it
        unsafe {
            let b_ptr = self
                .bytes
                .as_mut_ptr()
                .add(TRAILER_OFFSET + PREFIX_SIZE + RIGHT_SIBLING_OFFSET);
            write_u64_le_unsafe(b_ptr, right_sibling.into());
        }
    }

    // TODO: Continue implementing IndexPageMut - Need to be able to handle prefix compression runtime decisions
}
