//------------------------- Page specific types ------------------------------//

use super::index_cell::IndexCellOwned;
use super::index_cell::{IndexCellMut, IndexCellRef};
use super::slotted_page::{ENTRY_SIZE_U16, InsertErrorCtx, PAGE_SIZE_U16};
use crate::page::prefix_compression::find_prefix_offset;
use crate::page::{
    self, ENTRY_SIZE, HEADER_SIZE, PAGE_SIZE, PageError, SlotID, SlottedPageMut, SlottedPageRef,
    read_u16_le_unsafe,
};
use crate::page::{PageFlags, PageID, PageKind, PageStates, PageType, read_u64_le_unsafe};
use page::IndexLevel;
use page::slotted_page::SlotEntry;
use std::ops::Deref;
use std::path::Prefix;
use std::slice::from_raw_parts;
use std::u16::MAX;

pub(crate) type Result<T> = std::result::Result<T, InternalPageError>;

#[derive(Debug, Clone)]
pub(crate) enum InternalPageError {
    PageError(PageError),
    InvalidPageType,
    InvalidLevel,
    ChildNotFound,
}

impl From<PageError> for InternalPageError {
    fn from(error: PageError) -> Self {
        InternalPageError::PageError(error)
    }
}

const INDEX_SPECIAL_SIZE: u16 = 16;
const RIGHT_SIBLING_OFFSET: usize = 8;

// TODO: Integrate Level into rest of IndexPage

pub(crate) struct InternalPageMut<'page> {
    page: SlottedPageMut<'page>,
}

impl<'page> InternalPageMut<'page> {
    pub(crate) fn from_slotted_page(page: SlottedPageMut<'page>) -> Self {
        InternalPageMut { page }
    }

    // TODO: Check if this is correct
    pub(super) fn cell_mut_from_slot_entry(&mut self, se: SlotEntry) -> InternalCellMut<'_, 'page> {
        InternalCellMut {
            inner: IndexCellMut::from(&mut self.page, se),
        }
    }

    pub(crate) fn init_in_place(&mut self, lsn: u64) -> Result<()> {
        // We are given a slotted page from the allocator which we need to initialize
        // This we can assume is being done during a split or tree operation and therefore we must be efficient

        self.page.wipe_page();

        self.page
            .set_page_type(PageType::new(PageKind::IndexInternal as u8, 0).into());
        self.page.set_special_offset(INDEX_SPECIAL_SIZE);

        // Set free start to default HEADER_SIZE

        self.page.set_free_start(HEADER_SIZE as u16);

        // Adjust free_end for special offset
        self.page.set_free_end(PAGE_SIZE_U16 - INDEX_SPECIAL_SIZE)?;

        // Set lsn
        self.page.set_lsn(lsn);

        Ok(())
    }

    pub(crate) fn get_page_type(&self) -> PageType {
        PageType::from(self.page.get_page_type())
    }

    pub(crate) fn set_page_type(&mut self, page_type: PageKind) {
        self.page.set_page_type(page_type.into())
    }

    pub(crate) fn kind(&self) -> PageKind {
        self.get_page_type().page_kind()
    }

    pub(crate) fn level(&mut self) -> IndexLevel {
        IndexLevel::from(self.get_page_type().page_sub_type())
    }

    pub(crate) fn set_level(&mut self, level: IndexLevel) {
        let mut new_pt = self.get_page_type();
        new_pt.set_subtype_page_bits(level.into());
        self.page.set_page_type(new_pt.into())
    }

    // Special methods

    pub(crate) fn set_right_sibling(&mut self, page_id: PageID) {
        // Could use unsafe but since we are an owned struct building a SlottedPage we don't have a lock
        // and no others are waiting for access.
        if let Ok(special) = self.page.get_special_mut() {
            special[RIGHT_SIBLING_OFFSET..RIGHT_SIBLING_OFFSET + 8]
                .copy_from_slice(page_id.into().to_le_bytes().as_ref());
        }
    }

    pub(crate) fn has_right_sibling(&self) -> bool {
        if let Ok(special) = self.page.get_special_ref() {
            special[RIGHT_SIBLING_OFFSET..RIGHT_SIBLING_OFFSET + 8] != [0u8; 8]
        } else {
            false
        }
    }

    // TODO: Test and see if returning R is ok here
    pub(super) fn with_first_key<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&[u8]),
    {
        let page_ref = self.page.as_ref();

        if self.has_right_sibling() {
            let se = page_ref.slot_dir_ref().get_slot_entry(SlotID(1))?;
            let cell = IndexCellRef::from(page_ref, se);
            Ok(f(cell.get_key()))
        } else {
            let se = page_ref.slot_dir_ref().get_slot_entry(SlotID(0))?;
            let cell = IndexCellRef::from(page_ref, se);
            Ok(f(cell.get_key()))
        }
    }

    pub(super) fn get_first_key(&self) -> Result<InternalCellRef<'_>> {
        let page_ref = self.page.as_ref();

        if self.has_right_sibling() {
            let se = page_ref.slot_dir_ref().get_slot_entry(SlotID(1))?;
            let cell = InternalCellRef {
                inner: IndexCellRef::from(page_ref, se),
            };
            Ok(cell)
        } else {
            let se = page_ref.slot_dir_ref().get_slot_entry(SlotID(0))?;
            let cell = InternalCellRef {
                inner: IndexCellRef::from(page_ref, se),
            };
            Ok(cell)
        }
    }

    pub(crate) fn try_insert(&mut self, key: &[u8], child_ptr: PageID) -> Result<()> {
        // TODO: Do we want to do prefix finding here? or pass in a prefix offset?

        // Find prefix offset and slice key
        let mut prefix_offset: u16 = 0;
        self.with_first_key(|first_key| {
            let offset = find_prefix_offset(key, first_key);
            debug_assert!(offset <= std::u16::MAX as usize);
            prefix_offset = offset as u16;
        })?;

        // Encode cell
        let cell = IndexCellOwned::new(key, 0, child_ptr);

        // We can check if we can insert the cell - if we error we can propagate the InsertErrorCtx back up to the tree to decide
        self.page.check_contiguous_insert(cell.deref())?;

        // Now we find the insert index
        // - Here

        // We can try to allocate a cell - if we get an error, we can propagate the InsertErrorCtx back to the tree for it decide on a strategy
        let entry = self.page.insert_cell(cell.deref(), 0)?;

        todo!("Finish")
    }
}

pub(crate) struct InternalPageRef<'page> {
    page: SlottedPageRef<'page>,
}

impl Drop for InternalPageRef<'_> {
    fn drop(&mut self) {
        drop(self);
    }
}

impl<'page> InternalPageRef<'page> {
    pub(crate) fn from_slotted_page(page: SlottedPageRef<'page>) -> Self {
        Self { page }
    }

    pub(super) fn cell_from_slot_entry(&'_ self, se: SlotEntry) -> InternalCellRef<'page> {
        InternalCellRef {
            inner: IndexCellRef::from(self.page, se),
        }
    }

    pub(super) fn cell_from_slot_id(&'_ self, idx: SlotID) -> Result<InternalCellRef<'page>> {
        let se = self.page.slot_dir_ref().get_slot_entry(idx)?;
        Ok(self.cell_from_slot_entry(se))
    }

    // TODO: This needs to take an ordering function ptr
    pub(crate) fn find_child_ptr(&self, key: &[u8]) -> Result<PageID> {
        let mut high_key = false;
        if self.has_right_sibling() {
            //TODO: - For now we are returning wrapped PageError. We may want to handle the PageError differently and give a wrapped error with context
            let hkc = self.cell_from_slot_id(SlotID(0))?;
            // TODO: Fix this
            let high_key_cell = InternalCellRef::from(hkc);
            high_key = true;
            if key > high_key_cell.get_key() {
                return self.get_right_sibling();
            }
        };

        let skip = if high_key { 1 } else { 0 };

        // TODO: We can do a check on slot_count if it's above a threshold to use binary search

        // We need to store the last child_ptr so if we are not the rightmost child and we are greater than the last key,
        // we can 'fall off' to the last child_ptr
        let mut last_child_ptr: Option<PageID> = None;

        for se in self.page.slot_dir_ref().iter().skip(skip) {
            let cell = self.cell_from_slot_entry(se);
            last_child_ptr = Some(cell.get_child_ptr());
            let cell_key = cell.get_key();
            if key < cell_key {
                return Ok(cell.get_child_ptr());
            }
        }

        // Fall off to last chil_ptr here
        return last_child_ptr.ok_or(InternalPageError::ChildNotFound);
    }

    //

    pub(super) fn has_right_sibling(&self) -> bool {
        if let Ok(special) = self.page.get_special_ref() {
            special[RIGHT_SIBLING_OFFSET..RIGHT_SIBLING_OFFSET + 8] != [0u8; 8]
        } else {
            false
        }
    }

    pub(super) fn get_page_type(&self) -> PageType {
        PageType::from(self.page.get_page_type())
    }

    pub(crate) fn kind(&self) -> PageKind {
        self.get_page_type().page_kind()
    }

    pub(crate) fn flags(&self) -> PageFlags {
        PageFlags::from(self.page.get_flags())
    }

    pub(super) fn level(&self) -> IndexLevel {
        IndexLevel::from(self.get_page_type().page_sub_type())
    }

    pub(super) fn get_right_sibling(&self) -> Result<PageID> {
        let special = self.page.get_special_ref()?;
        // TODO Add safety info
        unsafe {
            let b_ptr = special.as_ptr().add(RIGHT_SIBLING_OFFSET);
            let sib = read_u64_le_unsafe(b_ptr);
            return Ok(sib.into());
        }
    }
}

//------------------ Internal Cells  ---------------------//

#[derive(Debug)]
pub(super) struct InternalCellRef<'page> {
    inner: IndexCellRef<'page>,
    // May want things like child_ptr or key unless we copy out and return on method call (think about why
    // we would want to store anything)
}

impl<'page> InternalCellRef<'page> {
    pub(super) fn get_key(&self) -> &[u8] {
        // TODO: Fix the error returning here
        self.inner.get_key()
    }

    pub(super) fn get_child_ptr(&self) -> PageID {
        // TODO: Fix the error returning here
        // TODO: Think about any checks specific to internal page cells we need to
        self.inner.get_value_ptr()
    }

    pub(super) fn get_prefix(&self) -> u16 {
        // TODO: Fix the error returning here
        // TODO: Think about any checks specific to internal page cells we need to
        self.inner.get_prefix()
    }
}

pub(super) struct InternalCellMut<'a, 'page> {
    inner: IndexCellMut<'a, 'page>,
}

impl<'a, 'page> InternalCellMut<'a, 'page> {
    fn get_key(&self) -> &[u8] {
        // TODO: Fix the error returning here
        self.inner.get_key()
    }

    fn get_child_ptr(&self) -> PageID {
        // TODO: Fix the error returning here
        // TODO: Think about any checks specific to internal page cells we need to
        self.inner.get_value_ptr()
    }

    fn get_prefix(&self) -> u16 {
        // TODO: Fix the error returning here
        // TODO: Think about any checks specific to internal page cells we need to
        self.inner.get_prefix()
    }

    // TODO: Make set_prefix method() once we have one on IndexCellMut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::{PageStates, RawPage};

    // TODO: Define assertions for the test
    #[test]
    fn getting_internal_cell() {
        let mut page: RawPage = [0u8; PAGE_SIZE];

        let mut sp = SlottedPageMut::init_new(
            &mut page,
            PageType::new(PageKind::IndexInternal as u8, 0).into(),
        );

        // Add a test cell
        let cell = IndexCellOwned::new("hello there".as_bytes(), 1, PageID(0));
        sp.insert_cell(cell.deref(), 0).ok().unwrap();

        drop(sp);

        let internal = InternalPageRef::from_slotted_page(SlottedPageRef::from_bytes(&page));

        let internal_cell = internal.cell_from_slot_id(SlotID(0)).ok().unwrap();
        let key = internal_cell.get_key();
        let result = String::from_utf8_lossy(key);
        assert_eq!(result, "hello there");
    }

    #[test]
    fn prefix_state_setting() {
        let mut page: RawPage = [0u8; PAGE_SIZE];

        let mut sp = SlottedPageMut::init_new(
            &mut page,
            PageType::new(PageKind::IndexInternal as u8, 0).into(),
        );

        sp.set_flags(PageStates::PrefixCompressed.bit());

        let internal = InternalPageRef::from_slotted_page(SlottedPageRef::from_bytes(&page));

        assert_eq!(
            internal.flags().has_flag(PageStates::PrefixCompressed),
            true
        );
    }

    // TODO: Define a test to insert a prefix compressed key using try_insert()
}
