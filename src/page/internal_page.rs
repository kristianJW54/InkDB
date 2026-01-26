//------------------------- Page specific types ------------------------------//

use super::index_cell::{IndexCellMut, IndexCellOwned, IndexCellRef};
use super::key_view::cmp_search;
use super::prefix_compression::find_prefix_offset;
use super::slotted_page::{
    ENTRY_SIZE_U16, InsertErrorCtx, PAGE_SIZE_U16, SIBLING_SPECIAL_SIZE_U16, SlotEntry,
};
use super::{
    ENTRY_SIZE, HEADER_SIZE, IndexLevel, InsertCtx, PAGE_SIZE, PageError, SlotID, SlottedPageMut,
    SlottedPageRef, read_u16_le_unsafe,
};
use super::{PageFlags, PageID, PageKind, PageStates, PageType, read_u64_le_unsafe};
use crate::tree::CompareFn;
use std::cmp::Ordering;
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
    InsertionIndexNotFound,
    TryInsertionFailed,
}

impl From<PageError> for InternalPageError {
    fn from(error: PageError) -> Self {
        InternalPageError::PageError(error)
    }
}

pub(super) const RIGHT_SIBLING_OFFSET: usize = 8;

// TODO: Integrate Level into rest of IndexPage

pub(crate) struct InternalPageMut<'page> {
    page: SlottedPageMut<'page>,
}

impl<'page> InternalPageMut<'page> {
    pub(crate) fn from_slotted_page(page: SlottedPageMut<'page>) -> Self {
        Self { page }
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

        // Set free start to default HEADER_SIZE

        self.page.set_free_start(HEADER_SIZE as u16);

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

    pub(crate) fn flags(&self) -> PageFlags {
        PageFlags::from(self.page.get_flags())
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

    // TODO: This needs to change to reference the special sibling space
    pub(crate) fn set_right_sibling(&mut self, page_id: PageID) {
        todo!()
    }

    // TODO: This needs to change to reference the special sibling space
    pub(crate) fn has_right_sibling(&self) -> bool {
        todo!()
    }

    fn get_prefix_key(&self) -> IndexCellRef {
        let page_ref = self.page.as_ref();
        let entry = page_ref.get_prefix_entry();
        IndexCellRef::from(page_ref, entry)
    }

    fn find_insertion_index(&self, key: &[u8]) -> Result<SlotID> {
        // TODO: We need to implement a binary search on slotted page and use it above a threshold for slot_count

        let page_ref = self.page.as_ref();

        for (i, se) in page_ref.slot_dir_ref().iter().enumerate() {
            let cell = IndexCellRef::from(page_ref, se);

            // The comparison key is a full key which has been encoded for bytewise comparison
            // therefore we need to get a KeyView of the current iteration key and compare it with the search key

            match cmp_search(key, cell.get_key_view()) {
                Ordering::Less => return Ok(SlotID(i as u16)),
                Ordering::Equal => return Ok(SlotID(i as u16)),
                Ordering::Greater => continue,
            }
        }

        Ok(SlotID(self.page.get_slot_count() as u16))
    }

    // TODO: Need to test new prefix implementation
    pub(crate) fn prepare_index_cell(
        &self,
        key: &[u8],
        child_ptr: PageID,
    ) -> Result<IndexCellOwned> {
        // Prepare the index cell for insertion into page
        // Here we define the boundary for checks such as prefix compression and whether or not we can compress the key
        // Then we create an IndexCellOwned cell to return

        // Get flags
        if PageFlags::has_flag(&self.flags(), PageStates::PrefixCompressed) {
            println!("yay");
            // We can compress the key
            // TODO: We simply need to get the prefix entry and compare it to the key
            // If the prefix offset is none then we error as we have mismatched page logic
            let prefix_key = self.get_prefix_key();
            let offset = find_prefix_offset(key, prefix_key.get_key());
            debug_assert!(offset <= std::u16::MAX as usize);

            let suffix = &key[offset..];
            println!("key: {:?}", key);
            println!("suffix: {:?}", suffix);
            return Ok(IndexCellOwned::new(suffix, offset as u16, child_ptr));
        } else {
            // We cannot compress the key
            Ok(IndexCellOwned::new(key, 0, child_ptr))
        }
    }

    // TODO: Change this to work with the new prefix model
    pub(crate) fn try_insert(&mut self, key: &[u8], child_ptr: PageID) -> Result<()> {
        let prepared_cell = self.prepare_index_cell(key, child_ptr)?;

        // We check if we can insert before finding an index to insert into
        // If we error we can propagate the InsertErrorCtx back up to the tree to decide
        self.page.check_contiguous_insert(prepared_cell.deref())?;
        //
        // Now we find the insert index
        let insertion_index = self.find_insertion_index(prepared_cell.deref())?;
        println!("insertion_index: {:?}", insertion_index);

        println!("prepared {:?}", prepared_cell);

        // Can insert now
        let ctx = InsertCtx {
            cell: prepared_cell,
            value_ptr: child_ptr.0,
            insert_index: insertion_index.0,
        };
        self.insert(ctx)
    }

    fn insert(&mut self, ctx: InsertCtx) -> Result<()> {
        // We can try to allocate a cell - if we get an error, we can propagate the InsertErrorCtx back to the tree for it decide on a strategy
        self.page.insert_cell(ctx.cell.deref(), ctx.insert_index)?;
        Ok(())
    }

    // ----------------------------
    // Handle splits and transfers
    // TODO: Need to implement split page methods for rebuilding pages and maintaining compression
    // Does it belong to this layer?
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

        // NOTE: This iteration would be an abstracted method handled by slotted page where a threshold is used to determine the strategy iteration/binary search
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
        todo!()
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
        todo!()
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
            0,
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
            0,
        );

        sp.set_flags(PageStates::PrefixCompressed.bit());

        let internal = InternalPageRef::from_slotted_page(SlottedPageRef::from_bytes(&page));

        assert_eq!(
            internal.flags().has_flag(PageStates::PrefixCompressed),
            true
        );
    }

    // TODO: Clean up this test and make clear assertions
    // TODO: Define a further test to find edge cases
    #[test]
    fn insert_prefix_compressed_key() {
        let mut page: RawPage = [0u8; PAGE_SIZE];
        let mut sp = SlottedPageMut::init_new(
            &mut page,
            PageType::new(PageKind::IndexInternal as u8, 0).into(),
            0,
        );

        sp.set_flags(PageStates::PrefixCompressed.bit());

        // Get internal page and insert a standard key so another key can be compressed against
        // Start with standard key
        sp.insert_cell_append(IndexCellOwned::new("00000123".as_bytes(), 0, PageID(1)).deref())
            .ok()
            .unwrap();
        // Now we need to insert a cell using try insert on the page specific layer so we can find the prefix offset and insert the compressed key
        let mut internal = InternalPageMut::from_slotted_page(sp);
        let result = internal.try_insert("00000456".as_bytes(), PageID(2));
        if result.is_ok() {
            let ref_internal =
                InternalPageRef::from_slotted_page(SlottedPageRef::from_bytes(&page));
            let key = ref_internal.cell_from_slot_id(SlotID(1)).ok().unwrap();
            assert_eq!(key.get_key(), "456".as_bytes());
        } else {
            println!("Failed to insert key");
        }

        // If we try to insert another key with a different prefix we should get an adjusted compressed key
        let mut internal =
            InternalPageMut::from_slotted_page(SlottedPageMut::from_bytes(&mut page));
        let result = internal.try_insert("00100678".as_bytes(), PageID(3));
        if result.is_ok() {
            let ref_internal =
                InternalPageRef::from_slotted_page(SlottedPageRef::from_bytes(&page));
            let key = ref_internal.cell_from_slot_id(SlotID(1)).ok().unwrap();
            assert_eq!(key.get_key(), "100678".as_bytes());
        } else {
            println!("Failed to insert key");
        }
    }

    #[test]
    fn find_insert_index() {
        let mut page: RawPage = [0u8; PAGE_SIZE];
        let mut sp = SlottedPageMut::init_new(&mut page, PageKind::IndexInternal as u8, 0);
        sp.set_flags(PageStates::PrefixCompressed.bit());

        // Insert [a, b, d, e]
        sp.insert_cell_append(IndexCellOwned::new("a".as_bytes(), 0, PageID(1)).deref())
            .ok()
            .unwrap();
        sp.insert_cell_append(IndexCellOwned::new("b".as_bytes(), 0, PageID(2)).deref())
            .ok()
            .unwrap();
        sp.insert_cell_append(IndexCellOwned::new("d".as_bytes(), 0, PageID(3)).deref())
            .ok()
            .unwrap();
        sp.insert_cell_append(IndexCellOwned::new("e".as_bytes(), 0, PageID(4)).deref())
            .ok()
            .unwrap();

        //

        //
        let internal = InternalPageMut::from_slotted_page(sp);
        let index = internal.find_insertion_index("e".as_bytes());
        match index {
            Ok(index) => println!("Index: {:?}", index),
            Err(err) => println!("Error: {:?}", err),
        }
    }
}
