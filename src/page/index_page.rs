use crate::page::{PageFlags, PageStates, slotted_page::HEADER_SIZE_U16};

use super::{PageKind, PageType, SlottedPageMut};

pub(super) enum IndexPageError {
    //
}

/// IndexPageMut holds the SlottedPageMut which is under the lifetime of the guard, given out by the PageFrame
/// After we perform operations on a page, IndexPage wrappers are dropped and SlottedPage is returned back
/// to the guard - so for every operation on a page, we must derive the correct type through meta data.
/// It is for this reason that we keep type state here only for correctness when performing intialisations, splitting,
/// merging, and transformations.
pub(super) struct IndexPageMut<'page> {
    bytes: SlottedPageMut<'page>,
}

impl<'page> IndexPageMut<'page> {
    pub(super) fn from_slotted_page(page: SlottedPageMut<'page>) -> Self {
        Self { bytes: page }
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

    // TODO: Continue implementing IndexPageMut - Need to be able to handle prefix compression runtime decisions
}
