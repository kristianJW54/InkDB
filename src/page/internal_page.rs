//------------------------- Page specific types ------------------------------//

// We want to look at fences - look at prefix compression and look ahead

use super::index_cell::IndexCellOwned;
use crate::page::index_cell::{IndexCellMut, IndexCellRef};
use crate::page::slotted_page::{ENTRY_SIZE_U16, InsertErrorCtx, PAGE_SIZE_U16, SlotEntry};
// Page types interpret over the slotted page for their type
use crate::page::{
    self, ENTRY_SIZE, HEADER_SIZE, PAGE_SIZE, PageError, SlottedPageMut, SlottedPageRef,
    read_u16_le_unsafe,
};
use crate::page::{PageID, PageKind, PageType, SlotEntry, read_u64_le_unsafe};
use page::IndexLevel;
use std::ops::Deref;
use std::slice::from_raw_parts;

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

pub(crate) struct IndexPageMut<'page> {
    page: SlottedPageMut<'page>,
}

impl<'page> IndexPageMut<'page> {
    pub(crate) fn from_slotted_page(page: SlottedPageMut<'page>) -> Self {
        IndexPageMut { page }
    }

    // TODO: Check if this is correct
    pub(super) fn cell_mut_from_slot_entry(
        &'page mut self,
        se: SlotEntry,
    ) -> InternalCellMut<'page> {
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

    pub(crate) fn try_insert(&mut self, key: &[u8], child_ptr: PageID) -> Result<()> {
        // Encode cell
        let cell = IndexCellOwned::new(key, child_ptr);

        // Fast path memory check
        let contiguous = self.page.free_contiguous_space();
        let frag = self.page.free_fragmented_space();

        // We need to figure logic here - do we want to return ctx if no contiguous and let b-tree call back in for frag?
        // Or do a frag check and return ctx with error for tree either can_compact or must split

        // We can check if we can insert the cell - if we error we can propagate the InsertErrorCtx back up to the tree to decide
        // If we are ok then we can find an insert index and try to insert the cell

        self.page.check_contiguous_insert(cell.deref())?;

        // Now we find the insert index
        // - Here

        // We can try to allocate a cell - if we get an error, we can propagate the InsertErrorCtx back to the tree for it decide on a strategy
        let entry = self.page.insert_cell(cell.deref(), 0)?;

        todo!("Finish")
    }
}

pub(crate) struct IndexPageRef<'page> {
    page: SlottedPageRef<'page>,
}

impl Drop for IndexPageRef<'_> {
    fn drop(&mut self) {
        drop(self);
    }
}

impl<'page> IndexPageRef<'page> {
    pub(crate) fn from_slotted_page(page: SlottedPageRef<'page>) -> Self {
        Self { page }
    }

    pub(super) fn cell_from_slot_entry(&'page self, se: SlotEntry) -> InternalCellRef<'page> {
        InternalCellRef {
            inner: IndexCellRef::from(&self.page, se),
        }
    }

    pub(crate) fn find_child_ptr(&self, key: &[u8]) -> Result<PageID> {
        let mut high_key = false;
        if self.has_right_sibling() {
            //TODO: - For now we are returning wrapped PageError. We may want to handle the PageError differently and give a wrapped error with context
            let hkc = self.page.cell_slice_from_id(SlotEntry(0))?;
            // TODO: Fix this
            let high_key_cell = InternalCellRef::from(hkc);
            high_key = true;
            if key > high_key_cell.get_key() {
                return self.get_right_sibling();
            }
        };

        let skip = if high_key { 1 } else { 0 };

        // TODO: ------- We can do a check on slot_count if it's above a threshold to use binary search

        // We need to store the last child_ptr so if we are not the rightmost child and we are greater than the last key,
        // we can 'fall off' to the last child_ptr
        let mut last_child_ptr: Option<PageID> = None;

        for se in self.page.slot_dir_ref().iter().skip(skip) {
            // TODO: Implement this method
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

    pub(crate) fn has_right_sibling(&self) -> bool {
        if let Ok(special) = self.page.get_special_ref() {
            special[RIGHT_SIBLING_OFFSET..RIGHT_SIBLING_OFFSET + 8] != [0u8; 8]
        } else {
            false
        }
    }

    pub(crate) fn get_page_type(&self) -> PageType {
        PageType::from(self.page.get_page_type())
    }

    pub(crate) fn kind(&self) -> PageKind {
        self.get_page_type().page_kind()
    }

    pub(crate) fn level(&self) -> IndexLevel {
        IndexLevel::from(self.get_page_type().page_sub_type())
    }

    pub(crate) fn get_right_sibling(&self) -> Result<PageID> {
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

// TODO: Fix the lifetimes and field names
#[derive(Debug)]
struct InternalCellRef<'a, 'page> {
    inner: IndexCellRef<'a, 'page>,
    // May want things like child_ptr or key unless we copy out and return on method call (think about why
    // we would want to store anything)
}

impl<'a, 'page> InternalCellRef<'a, 'page> {
    fn get_key(&self) -> &[u8] {
        // TODO: Fix the error returning here
        self.inner.get_key().ok().unwrap()
    }

    fn get_child_ptr(&self) -> PageID {
        // TODO: Fix the error returning here
        // TODO: Think about any checks specific to internal page cells we need to
        self.inner.get_value_ptr().ok().unwrap()
    }

    fn get_prefix(&self) -> u16 {
        // TODO: Fix the error returning here
        // TODO: Think about any checks specific to internal page cells we need to
        self.inner.get_prefix().ok().unwrap()
    }
}

// TODO: Fix the lifetimes and field names
pub(super) struct InternalCellMut<'a, 'page> {
    inner: IndexCellMut<'a, 'page>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_do() {}
}
