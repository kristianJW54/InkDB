


//------------------------- Page specific types ------------------------------//

// Page types interpret over the slotted page for their type

use std::fmt::Error;
use crate::page::page_frame::{PageReadGuard, PageWriteGuard};
use crate::page::PageID;

const INDEX_ROOT: u8 = 0b0001;
const INDEX_INTERNAL: u8 = INDEX_ROOT << 1;
const INDEX_LEAF: u8 = INDEX_ROOT << 2;

// TODO Bit operator functions

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum IndexRole {
    Root,
    Internal,
    Leaf,
}

impl IndexRole {
    pub(crate) fn from_bits(bits: u8) -> Self {
        match bits {
            INDEX_ROOT => IndexRole::Root,
            INDEX_INTERNAL => IndexRole::Internal,
            INDEX_LEAF => IndexRole::Leaf,
            _ => IndexRole::Leaf,
        }
    }
    pub(crate) fn to_bits(&self) -> u8 {
        match self {
            IndexRole::Root => INDEX_ROOT,
            IndexRole::Internal => INDEX_INTERNAL,
            IndexRole::Leaf => INDEX_LEAF,
        }
    }
}

pub(crate) struct IndexPageRef<'page> {
    data: PageReadGuard<'page>,
}

impl <'page> IndexPageRef<'page> {

    pub(crate) fn is_leaf(&self) -> bool {
        self.data.get_flags() & INDEX_LEAF != 0
    }

    pub(crate) fn is_internal(&self) -> bool {
        self.data.get_flags() & INDEX_INTERNAL != 0
    }

    pub(crate) fn is_root(&self) -> bool {
        self.data.get_flags() & INDEX_ROOT != 0
    }

    pub(crate) fn get_index_type(&self) -> IndexRole {
        IndexRole::from_bits(self.data.get_flags())
    }

    pub(crate) fn from_guard(guard: PageReadGuard<'page>) -> Self { Self { data: guard } }

    pub(crate) fn find_child_ptr(&self, key: &[u8]) -> Result<Option<PageID>, Error> {

        for se in self.data.slot_dir_ref().iter() {
            if let Ok(cell) = self.data.cell_slice_from_entry(se) {
                // Get key, compare and return child_ptr
                
            }

        }

        todo!("finish find_child_ptr")
    }

    //

}

//TODO Later we decide if we want LeafIndexRef/Mut and InternalIndexRef/Mut etc
// May not be needed at all...

#[test]
fn test_bit() {

    println!("{}", INDEX_INTERNAL);
    println!("{}", INDEX_LEAF);

}