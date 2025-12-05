


//------------------------- Page specific types ------------------------------//

// Page types interpret over the slotted page for their type

use std::fmt::Error;
use crate::page::page_frame::{PageReadGuard, PageWriteGuard};
use crate::page::{PageID, PageKind, PageType};
use crate::page::raw_page::SlottedPage;


// TODO Add Index Flags like DELETED, HALF_DEAD, INCOMPLETE_SPLIT

// TODO Add Page Sub Type enum

const INDEX_SPECIAL_SIZE: u16 = size_of::<IndexTail>() as u16;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IndexTail {
    right_sibling: u64,
    left_sibling: u64,
}

// Levels for the index page

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndexLevel(u8);

impl IndexLevel {

    pub(crate) const MAX: u8 = 15;

    pub(crate) fn new(level: u8) -> Self {
        assert!(level <= Self::MAX, "Max level for bit field is 15");
        Self(level)
    }

    pub(crate) fn get(self) -> u8 {
        self.0
    }

}

// TODO Integrate Level into rest of IndexPage


pub(crate) struct IndexPageOwned(SlottedPage);

// TODO Implement IndexPageOwned
impl IndexPageOwned {
    pub(crate) fn new(lsn: u64) -> Self {
        let mut page = SlottedPage::new_blank();
        page.set_page_type(PageType::new(PageKind::Index as u8, 0).into());
        page.set_special_offset(INDEX_SPECIAL_SIZE);

        // We now need to get the special space and modify

        Self(page)
    }

    pub(crate) fn into_inner(self) -> SlottedPage {
        self.0
    }

    pub(crate) fn get_page_type(&self) -> PageType {
        PageType::from(self.0.get_page_type())
    }

    pub(crate) fn kind(&self) -> PageKind {
        PageType::from(self.0.get_page_type()).page_kind()
    }


}



pub(crate) struct IndexPageRef<'page> {
    data: PageReadGuard<'page>,
}

impl <'page> IndexPageRef<'page> {

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

    // TODO Implement page type methods

}

//TODO Later we decide if we want LeafIndexRef/Mut and InternalIndexRef/Mut etc
// May not be needed at all...


//------------------ Index Tuples ---------------------//

// An index tuple is similar to Postgres Index tuple which is both a pivot tuple (internal) and
// leaf tuple (leaf) with TID pointer to heap data


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_special_space() {

        let index_page = IndexPageOwned::new(0);
        let mut page = index_page.into_inner();
        let space = page.get_special_mut().unwrap();
        println!("special space = {:?}", space);

    }

    #[test]
    fn index_page_type() {

        let index_page = IndexPageOwned::new(0);
        println!("page type = {:?}", index_page.kind());

    }

}
