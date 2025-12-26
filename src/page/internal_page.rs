//------------------------- Page specific types ------------------------------//

// We want to look at fences - look at prefix compression and look ahead

// Page types interpret over the slotted page for their type
use crate::page::{
    self, ENTRY_SIZE, HEADER_SIZE, PAGE_SIZE, PageError, SlottedPageMut, SlottedPageRef,
    read_u16_le_unsafe,
};
use crate::page::{PageID, PageKind, PageType, SlotID, read_u64_le_unsafe};
use page::IndexLevel;
use std::ops::Deref;
use std::slice::from_raw_parts;

pub(crate) type Result<T> = std::result::Result<T, IndexPageError>;

#[derive(Debug, Clone)]
pub(crate) enum IndexPageError {
    PageError(PageError),
    InvalidPageType,
    InvalidLevel,
}

impl From<PageError> for IndexPageError {
    fn from(error: PageError) -> Self {
        IndexPageError::PageError(error)
    }
}

const INDEX_SPECIAL_SIZE: u16 = 16;
const RIGHT_SIBLING_OFFSET: usize = 8;

// TODO Integrate Level into rest of IndexPage

pub(crate) struct IndexCellOwned(Box<[u8]>);

impl IndexCellOwned {
    pub(crate) const MAX_INDEX_CELL_SIZE: usize = PAGE_SIZE - HEADER_SIZE - ENTRY_SIZE;

    pub(crate) fn new(key: &[u8], child_ptr: PageID) -> Self {
        let est_size = 10 + key.len();
        assert!(est_size < Self::MAX_INDEX_CELL_SIZE);

        let mut cell = Vec::with_capacity(est_size);
        cell.extend_from_slice(&child_ptr.into().to_le_bytes());
        cell.extend_from_slice(&(key.len() as u16).to_le_bytes());
        cell.extend_from_slice(key);
        IndexCellOwned(cell.into_boxed_slice())
    }
}

impl Deref for IndexCellOwned {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) struct IndexPageMut<'page> {
    page: SlottedPageMut<'page>,
}

impl<'page> IndexPageMut<'page> {
    pub(crate) fn from_slotted_page(page: SlottedPageMut<'page>) -> Self {
        IndexPageMut { page }
    }

    pub(crate) fn init_in_place(&mut self, lsn: u64) -> Result<()> {
        // We are given a slotted page from the allocator which we need to initialize
        // This we can assume is being done during a split or tree operation and therefore we must be efficient

        self.page.wipe_page();

        self.page
            .set_page_type(PageType::new(PageKind::IndexInternal as u8, 0).into());
        self.page.set_special_offset(INDEX_SPECIAL_SIZE);

        // Set free start to default HEADER_SIZE

        self.page.set_free_start(HEADER_SIZE);

        // Adjust free_end for special offset
        self.page
            .set_free_end(PAGE_SIZE as u16 - INDEX_SPECIAL_SIZE)?;

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

    pub(crate) fn add_cell_append_slot_entry(&mut self, cell: IndexCellOwned) -> Result<()> {
        // We take an owned IndexCell which we then consume and store as bytes
        let bytes = cell.0.as_ref();
        self.page.add_cell_append_slot_entry(bytes)?;
        Ok(())
    }

    pub(crate) fn add_cell_at_slot_entry_index(
        &mut self,
        index: usize,
        cell: IndexCellOwned,
    ) -> Result<()> {
        // We take an owned IndexCell which we then consume and store as bytes in the RawPage
        let bytes = cell.deref();
        self.page.add_cell_at_slot_entry_index(index, bytes)?;
        Ok(())
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

    pub(crate) fn find_child_ptr(&self, key: &[u8]) -> Result<Option<PageID>> {
        let mut high_key = false;
        if self.has_right_sibling() {
            //TODO - For now we are returning wrapped PageError. We may want to handle the PageError differently
            // and give a wrapped error with context
            let hkc = self.page.cell_slice_from_id(SlotID(0))?;
            let high_key_cell = IndexCell::from(hkc);
            high_key = true;
            if key > high_key_cell.get_key() {
                return Ok(self.get_right_sibling());
            }
        };

        let skip = if high_key { 1 } else { 0 };

        for se in self.page.slot_dir_ref().iter().skip(skip) {
            let cell = IndexCell::from(self.page.cell_slice_from_entry(se));
            let cell_key = cell.get_key();
            if key < cell_key {
                return Ok(Some(cell.get_value_ptr()));
            }
        }
        Ok(None)
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

    pub(crate) fn get_right_sibling(&self) -> Option<PageID> {
        let special = self.page.get_special_ref().ok()?;
        // TODO Add safety info
        unsafe {
            let b_ptr = special.as_ptr().add(RIGHT_SIBLING_OFFSET);
            let sib = read_u64_le_unsafe(b_ptr);
            return if sib == 0 { None } else { Some(sib.into()) };
        }
    }
}

//TODO Later we decide if we want LeafIndexRef/Mut and InternalIndexRef/Mut etc
// May not be needed at all...

//------------------ Index Cells & Tuples ---------------------//

// An index tuple is similar to Postgres Index tuple which is both a pivot tuple (internal) and
// leaf tuple (leaf) with TID pointer to heap data

// Index Cell Layout:
// child_ptr OR tid_ptr (8 bytes) | key_len (2 bytes) | key_data |

const CHILD_PTR_OFFSET: usize = 0;
const KEY_LEN_OFFSET: usize = 8;
const KEY_DATA_OFFSET: usize = 10;

// TODO Finish completing the index cell layout and method blocks

#[derive(Debug, Clone, Copy, PartialEq)]
struct IndexCell<'index_page> {
    cell: &'index_page [u8],
    // May want things like child_ptr or key unless we copy out and return on method call (think about why
    // we would want to store anything)
}

impl<'index_page> IndexCell<'index_page> {
    fn from(cell_ref: &'index_page [u8]) -> Self {
        assert!(cell_ref.len() >= 10);
        Self { cell: cell_ref }
    }

    fn get_key(&self) -> &[u8] {
        unsafe {
            let cell_ptr = self.cell.as_ptr();
            let key_len = read_u16_le_unsafe(cell_ptr.add(KEY_LEN_OFFSET)) as usize;

            let key_ptr = cell_ptr.add(KEY_DATA_OFFSET);

            debug_assert!(KEY_DATA_OFFSET + key_len <= self.cell.len());

            from_raw_parts(key_ptr, key_len)
        }
    }

    fn get_value_ptr(&self) -> PageID {
        // SAFETY: The cell is guaranteed to be at least 10 bytes long, and the child pointer is at offset 0.
        unsafe {
            let cell_ptr = self.cell.as_ptr().add(CHILD_PTR_OFFSET);
            let page_id = read_u64_le_unsafe(cell_ptr);
            PageID::from(page_id)
        }
    }
}
