use super::slotted_page::SlotEntry;
use super::{
    ENTRY_SIZE, HEADER_SIZE, PAGE_SIZE, PageError, PageID, SlottedPageMut, SlottedPageRef,
    read_u16_le_unsafe, read_u64_le_unsafe, write_u16_le_unsafe,
};
use std::ops::Deref;
use std::slice::from_raw_parts;
//
// Similar to PostGres PivotCell where both internal pages and leaf pages share the same cell structure

// An index tuple is similar to Postgres Index tuple which is both a pivot tuple (internal) and
// leaf tuple (leaf) with TID pointer to heap data

// Index Cell Layout:
// |----------------------|----------------------|-----------------|------------|
// | child_ptr OR tid_ptr | prefix               | key_len         | key_data   |
// |----------------------|----------------------|-----------------|------------|
// | 8 bytes              | 2 bytes              | 2 bytes         | variable   |
// |----------------------|----------------------|-----------------|------------|

pub(super) type Result<T> = std::result::Result<T, IndexCellError>;

pub(super) enum IndexCellError {
    PageError(PageError),
    InvalidCellSize,
    InvalidChildPtr,
    InvalidKeyLen,
}

impl From<PageError> for IndexCellError {
    fn from(err: PageError) -> Self {
        IndexCellError::PageError(err)
    }
}

const CHILD_PTR_OFFSET: usize = 0;
const PREFIX_OFFSET: usize = 8;
const KEY_LEN_OFFSET: usize = 10;
const KEY_DATA_OFFSET: usize = 12;

pub(super) struct IndexCellOwned(Box<[u8]>);

// TODO: Need IndexCellOwned to be required for cell insertions at the page layer
impl IndexCellOwned {
    pub(crate) const MAX_INDEX_CELL_SIZE: usize = (PAGE_SIZE - HEADER_SIZE - ENTRY_SIZE) / 2;

    pub(crate) fn new(key: &[u8], prefix_offset: u16, child_ptr: PageID) -> Self {
        let est_size = 10 + key.len();
        assert!(est_size < Self::MAX_INDEX_CELL_SIZE);

        // NOTE: Can we slice instead of Vec?
        let mut cell = Vec::with_capacity(est_size);
        cell.extend_from_slice(&child_ptr.into().to_le_bytes());
        cell.extend_from_slice(&prefix_offset.to_le_bytes());
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

// TODO: Finish implementations for IndexCellRef and IndexCellMut

// IndexCellRef holds a reference to the slotted page under a index page lifetime, it defines methods and behaviours only for referencing cell data
// within the slotted page
#[derive(Debug)]
pub(super) struct IndexCellRef<'a, 'page> {
    cell: &'a SlottedPageRef<'page>,
    slot_entry: SlotEntry,
}

impl<'a, 'page> IndexCellRef<'a, 'page> {
    pub(super) fn from(page: &'a SlottedPageRef<'page>, slot: SlotEntry) -> Self {
        IndexCellRef {
            cell: page,
            slot_entry: slot,
        }
    }

    pub(super) fn get_key(&self) -> Result<&[u8]> {
        let cell = self.cell.cell_slice_from_entry(self.slot_entry);

        // SAFETY: The cell is guaranteed to be at least 12 bytes long, and the key data is at offset 10.
        unsafe {
            let cell_ptr = cell.as_ptr();
            let key_len = read_u16_le_unsafe(cell_ptr.add(KEY_LEN_OFFSET)) as usize;

            let key_ptr = cell_ptr.add(KEY_DATA_OFFSET);

            return Ok(from_raw_parts(key_ptr, key_len));
        }
    }
    pub(super) fn get_value_ptr(&self) -> Result<PageID> {
        let cell = self.cell.cell_slice_from_entry(self.slot_entry);

        // SAFETY: The cell is guaranteed to be at least 12 bytes long, and the child pointer is at offset 0.
        unsafe {
            let cell_ptr = cell.as_ptr().add(CHILD_PTR_OFFSET);
            let page_id = read_u64_le_unsafe(cell_ptr);
            Ok(PageID::from(page_id))
        }
    }

    pub(super) fn get_prefix(&self) -> Result<u16> {
        let cell = self.cell.cell_slice_from_entry(self.slot_entry);

        // SAFETY: The cell is guaranteed to be at least 12 bytes long, and the prefix is at offset 8.
        unsafe {
            let cell_ptr = cell.as_ptr().add(PREFIX_OFFSET);
            Ok(read_u16_le_unsafe(cell_ptr))
        }
    }
}

pub(super) struct IndexCellMut<'a, 'page> {
    cell: &'a mut SlottedPageMut<'page>,
    slot: SlotEntry,
}

impl<'a, 'page> IndexCellMut<'a, 'page> {
    pub(super) fn from(page: &'a mut SlottedPageMut<'page>, slot: SlotEntry) -> Self {
        IndexCellMut {
            cell: page,
            slot: slot,
        }
    }

    pub(super) fn get_key(&self) -> Result<&[u8]> {
        let cell = self.cell.cell_slice_from_entry(self.slot);

        // SAFETY: The cell is guaranteed to be at least 12 bytes long, and the key data is at offset 10.
        unsafe {
            let cell_ptr = cell.as_ptr();
            let key_len = read_u16_le_unsafe(cell_ptr.add(KEY_LEN_OFFSET)) as usize;

            let key_ptr = cell_ptr.add(KEY_DATA_OFFSET);

            debug_assert!(KEY_DATA_OFFSET + key_len <= cell.len());

            return Ok(from_raw_parts(key_ptr, key_len));
        }
    }
    pub(super) fn get_value_ptr(&self) -> Result<PageID> {
        let cell = self.cell.cell_slice_from_entry(self.slot);

        // SAFETY: The cell is guaranteed to be at least 12 bytes long, and the child pointer is at offset 0.
        unsafe {
            let cell_ptr = cell.as_ptr().add(CHILD_PTR_OFFSET);
            let page_id = read_u64_le_unsafe(cell_ptr);
            Ok(PageID::from(page_id))
        }
    }

    pub(super) fn get_prefix(&self) -> Result<u16> {
        let cell = self.cell.cell_slice_from_entry(self.slot);

        // SAFETY: The cell is guaranteed to be at least 12 bytes long, and the prefix is at offset 8.
        unsafe {
            let cell_ptr = cell.as_ptr().add(PREFIX_OFFSET);
            Ok(read_u16_le_unsafe(cell_ptr))
        }
    }

    // We only should be mutating the prefix - index keys are immutable and should not be changed unless they are removed and re-inserted.
    pub(super) fn set_prefix(&mut self, prefix: u16) -> Result<()> {
        // TODO: Implement the set_prefix method

        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Test IndexCell lifetimes

    #[test]
    fn mutate_variable() {
        let mut cell = [0u8; 5];

        #[derive(Debug)]
        struct MyStruct<'a> {
            var_cell: &'a mut [u8],
        }

        let var_cell = MyStruct {
            var_cell: &mut cell,
        };

        var_cell.var_cell[0] = 1;
        println!("Changing var_cell -> {:?}", var_cell);
        println!("Checking cell -> {:?}", cell);
    }
}
