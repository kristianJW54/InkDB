


//------------------------- Page specific types ------------------------------//

// Page types interpret over the slotted page for their type

use std::fmt::Error;
use crate::page::page_frame::{PageReadGuard, PageWriteGuard};
use crate::page::{PageID, PageType};
use crate::page::raw_page::SlottedPage;


// TODO Add Index Flags like DELETED, HALF_DEAD, INCOMPLETE_SPLIT

struct IndexTail {
    right_sibling: u64,
    left_sibling: u64,
}

pub(crate) struct IndexPageOwned {
    page: SlottedPage,
}

// TODO Implement IndexPageOwned
impl IndexPageOwned {
    pub(crate) fn new(lsn: u64) -> Self {
        let mut page = SlottedPage::new_blank();
        page.set_page_type(PageType::Index);
        page.set_special_offset(16);

        // We now need to get the special space and modify

        Self { page }

    }

    pub(crate) fn into_inner(self) -> SlottedPage {
        self.page
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
    fn creating_index_page() {

        let index_page = IndexPageOwned::new(0);
        let mut page = index_page.into_inner();
        let space = page.get_special_mut().unwrap();
        println!("special space = {:?}", space);

    }
}
