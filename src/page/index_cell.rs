use super::{ENTRY_SIZE, HEADER_SIZE, PAGE_SIZE, PageID};
use std::ops::Deref;
// Similar to PostGres PivotCell where both internal pages and leaf pages share the same cell structure

pub(super) struct IndexCellOwned(Box<[u8]>);

// Impl block here
impl IndexCellOwned {
    pub(crate) const MAX_INDEX_CELL_SIZE: usize = (PAGE_SIZE - HEADER_SIZE - ENTRY_SIZE) / 2;

    // TODO: Fix this signature to be generic - let the page type layers enforce the parameters
    pub(crate) fn new(key: &[u8], child_ptr: PageID) -> Self {
        let est_size = 10 + key.len();
        assert!(est_size < Self::MAX_INDEX_CELL_SIZE);

        // TODO: Use slice instead of Vec
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

// TODO: Finish implementations for IndexCellRef and IndexCellMut
pub(super) struct IndexCellRef<'a> {
    cell: &'a [u8],
}

pub(super) struct IndexCellMut<'a> {
    cell: &'a mut [u8],
}
