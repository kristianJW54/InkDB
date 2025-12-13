//------------------------- Page specific types ------------------------------//

// Page types interpret over the slotted page for their type
use crate::page::{PageError, SlottedPage, read_u16_le_unsafe};
use crate::page::{PageID, PageKind, PageType, SlotID, read_u64_le_unsafe};
use crate::page_cache::page_frame::{PageReadGuard, PageWriteGuard};
use std::ptr;
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

const INDEX_SPECIAL_SIZE: u16 = size_of::<IndexTail>() as u16;
const RIGHT_SIBLING_OFFSET: usize = 8;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IndexTail {
    right_sibling: PageID,
    left_sibling: PageID,
}

// Levels for the index page

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndexLevel(pub u8);

impl IndexLevel {
    pub(crate) const MAX: u8 = 15;

    pub(crate) fn new(level: u8) -> Self {
        assert!(level <= Self::MAX, "Max level for bit field is 15");
        Self(level)
    }

    pub(crate) fn into(self) -> u8 {
        self.0
    }
}

impl From<u8> for IndexLevel {
    fn from(value: u8) -> IndexLevel {
        IndexLevel::new(value)
    }
}

// TODO Integrate Level into rest of IndexPage

pub(crate) struct IndexCellOwned(Box<[u8]>);

impl IndexCellOwned {
    pub(crate) fn new(key: &[u8], child_ptr: PageID) -> Self {
        let mut cell = Vec::with_capacity(10 + key.len());
        cell[0..CHILD_PTR_OFFSET].copy_from_slice(&child_ptr.into().to_le_bytes());
        cell[CHILD_PTR_OFFSET..KEY_LEN_OFFSET].copy_from_slice(&(key.len() as u16).to_le_bytes());
        cell[KEY_LEN_OFFSET..].copy_from_slice(key);
        IndexCellOwned(cell.into_boxed_slice())
    }
}

pub struct IndexPageOwned(SlottedPage);

// TODO Implement IndexPageOwned
impl IndexPageOwned {
    pub(crate) fn new(lsn: u64) -> Self {
        let mut page = SlottedPage::new_blank();
        page.set_page_type(PageType::new(PageKind::Index as u8, 0).into());
        page.set_special_offset(INDEX_SPECIAL_SIZE);

        // Set lsn
        page.set_lsn(lsn);

        Self(page)
    }

    pub(crate) fn into_inner(self) -> SlottedPage {
        self.0
    }

    pub(crate) fn get_page_type(&self) -> PageType {
        PageType::from(self.0.get_page_type())
    }

    pub(crate) fn kind(&self) -> PageKind {
        self.get_page_type().page_kind()
    }

    pub(crate) fn level(&self) -> IndexLevel {
        IndexLevel::from(self.get_page_type().page_sub_type())
    }

    pub(crate) fn set_level(&mut self, level: IndexLevel) {
        let mut new_pt = self.get_page_type();
        new_pt.set_subtype_page_bits(level.into());
        self.0.set_page_type(new_pt.into())
    }

    // Special methods

    pub(crate) fn set_right_sibling(&mut self, page_id: PageID) {
        // Could use unsafe but since we are an owned struct building a SlottedPage we don't have a lock
        // and no others are waiting for access.
        if let Ok(special) = self.0.get_special_mut() {
            special[RIGHT_SIBLING_OFFSET..RIGHT_SIBLING_OFFSET + 8]
                .copy_from_slice(page_id.into().to_le_bytes().as_ref());
        }
    }

    pub(crate) fn has_right_sibling(&self) -> bool {
        if let Ok(special) = self.0.get_special_ref() {
            special[RIGHT_SIBLING_OFFSET..RIGHT_SIBLING_OFFSET + 8] != [0u8; 8]
        } else {
            false
        }
    }
    // TODO Finish
    pub(crate) fn add_cell_append_slot_entry(&mut self, cell: IndexCellOwned) -> Result<()> {
        // We take an owned IndexCell which we then consume and store as bytes
        //
        todo!("Implement add_cell_append_slot_entry")
    }
}

pub(crate) struct IndexPageRef<'page> {
    data: PageReadGuard<'page>,
}

impl<'page> IndexPageRef<'page> {
    pub(crate) fn from_guard(guard: PageReadGuard<'page>) -> Self {
        Self { data: guard }
    }

    pub(crate) fn find_child_ptr(&self, key: &[u8]) -> Result<Option<PageID>> {
        let mut high_key = false;
        if self.has_right_sibling() {
            //TODO - For now we are returning wrapped PageError. We may want to handle the PageError differently
            // and give a wrapped error with context
            let hkc = self.data.cell_slice_from_id(SlotID(0))?;
            let high_key_cell = IndexCell::from(hkc);
            high_key = true;
            if key > high_key_cell.get_key() {
                return Ok(self.get_right_sibling());
            }
        };

        // TODO Sort out high_key
        let skip = if high_key { 1 } else { 0 };

        for se in self.data.slot_dir_ref().iter().skip(skip) {
            let cell = IndexCell::from(self.data.cell_slice_from_entry(se));

            if key < cell.get_key() {
                return Ok(Some(cell.get_child_ptr()));
            }
        }
        Ok(None)
    }

    //

    pub(crate) fn has_right_sibling(&self) -> bool {
        if let Ok(special) = self.data.get_special_ref() {
            special[RIGHT_SIBLING_OFFSET..RIGHT_SIBLING_OFFSET + 8] != [0u8; 8]
        } else {
            false
        }
    }

    pub(crate) fn get_page_type(&self) -> PageType {
        println!("page_type = {}", self.data.get_page_type());
        PageType::from(self.data.get_page_type())
    }

    pub(crate) fn level(&self) -> IndexLevel {
        IndexLevel::from(self.get_page_type().page_sub_type())
    }

    pub(crate) fn get_right_sibling(&self) -> Option<PageID> {
        let special = self.data.get_special_ref().ok()?;
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

    fn get_child_ptr(&self) -> PageID {
        // SAFETY: The cell is guaranteed to be at least 10 bytes long, and the child pointer is at offset 0.
        unsafe {
            let cell_ptr = self.cell.as_ptr().add(CHILD_PTR_OFFSET);
            let page_id = read_u64_le_unsafe(cell_ptr);
            PageID::from(page_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_cache::page_frame::PageFrame;

    #[test]
    fn index_special_space() {
        let index_page = IndexPageOwned::new(0);
        let mut page = index_page.into_inner();
        let space = page.get_special_mut().unwrap();
        println!("special space = {:?}", space);
    }

    #[test]
    fn index_page_type() {
        let mut index_page = IndexPageOwned::new(0);
        println!("page type = {:?}", index_page.kind());
        println!("page level = {:?}", index_page.level());
        println!("has right sibling = {:?}", index_page.has_right_sibling());

        index_page.set_level(IndexLevel::new(2));
        index_page.set_right_sibling(PageID(1234));
        println!("page type = {:?}", index_page.kind());
        println!("page new level = {:?}", index_page.level());
        println!("has right sibling = {:?}", index_page.has_right_sibling());

        let page = index_page.into_inner();
        let frame = PageFrame::new_frame_from_page(PageID(0), PageKind::Index, page);
        let index_ref = IndexPageRef::from_guard(frame.page_read_guard());

        println!("index ref level = {:?}", index_ref.level());
        println!("page id = {:?}", index_ref.get_right_sibling());
    }

    #[test]
    fn find_child_ptr() {
        let mut index_page = IndexPageOwned::new(0);
        index_page.set_level(IndexLevel::new(2));
        index_page.set_right_sibling(PageID(1234));

        // TODO Need an add cell to the raw_page?
    }
}
