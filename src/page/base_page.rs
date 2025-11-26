

// NOTE: The raw slotted page

use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::RwLockReadGuard;
use crate::page::{PageID, PageType, RawPage};

// TODO If SlottedPage gets too chaotic with mutating and reading we can split into SlottedRead & SlottedWrite??

/*
SLOTTED PAGE is dumb - it only knows how to make structural changes to the universal base layout
| header | slot array growing upward | free space | cells growing downward |
*/

//--------------------- Header -------------------------//


// Header is usually 24 bytes long - Looking at Postgres

// -- Log Sequence Number: 8 bytes
// -- Checksum: 2 bytes
// -- Page Type: 1 byte
// -- Flag bit: 1 byte
// -- Free_start: 2 bytes
// -- Free_end  : 2 bytes
// -- Special_start: 2 bytes
// -- Size and Version: 2 bytes
// -- TransactionID: 4 bytes (Oldest unpruned XMAX on page)

const PAGE_SIZE: usize = 4096;

pub const LSN_OFFSET: usize = 0;
pub const LSN_SIZE: usize = 8;
pub const CHECKSUM_OFFSET: usize = LSN_OFFSET + LSN_SIZE;
pub const CHECKSUM_SIZE: usize = 2;
pub const PAGE_TYPE_OFFSET: usize = CHECKSUM_OFFSET + CHECKSUM_SIZE;
pub const PAGE_TYPE_SIZE: usize = 1;
pub const FLAGS_OFFSET: usize = PAGE_TYPE_OFFSET + PAGE_TYPE_SIZE;
pub const FLAGS_SIZE: usize = 1;
pub const FREE_START_OFFSET: usize = FLAGS_OFFSET + FLAGS_SIZE;
pub const FREE_START_SIZE: usize = 2;
pub const FREE_END_OFFSET: usize = FREE_START_OFFSET + FREE_START_SIZE;
pub const FREE_END_SIZE: usize = 2;
pub const SPECIAL_OFFSET: usize = FREE_END_OFFSET + FREE_END_SIZE;
pub const SPECIAL_SIZE: usize = 2;
pub const SIZE_VERSION_OFFSET: usize = SPECIAL_OFFSET + SPECIAL_SIZE;
pub const SIZE_VERSION_SIZE: usize = 2;
pub const TXID_OFFSET: usize = SIZE_VERSION_OFFSET + SIZE_VERSION_SIZE;
pub const TXID_SIZE: usize = 4;

const HEADER_SIZE: usize = TXID_OFFSET + TXID_SIZE;


#[derive(Debug)]
pub(crate) struct SlottedPage {
    bytes: RawPage,
}

impl Deref for SlottedPage {
    type Target = RawPage;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl DerefMut for SlottedPage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

impl Default for SlottedPage {
    fn default() -> Self { Self { bytes: [0u8; 4096] } }
}

impl SlottedPage {
    pub fn mutate_first_byte(&mut self) {
        self.bytes[0] = 1
    }

    pub fn get_first_byte(&self) -> usize {
        self.bytes[0] as usize
    }

    // Start of real methods

    //NOTE: The new method needs to take parameters from the allocator like lsn, checksum etc
    pub(crate) fn new(lsn: u64, page_type: PageType) -> Self {
        let mut buff = [0u8; 4096];
        buff[0..0+LSN_SIZE].copy_from_slice(lsn.to_le_bytes().as_slice());
        let b = page_type as u8;
        buff[PAGE_TYPE_OFFSET] = b;

        // We must set the offsets to a default
        buff[FREE_START_OFFSET..FREE_START_OFFSET + FREE_START_SIZE].copy_from_slice((HEADER_SIZE as u16).to_le_bytes().as_slice());
        buff[FREE_END_OFFSET..FREE_END_OFFSET + FREE_END_SIZE].copy_from_slice(((PAGE_SIZE - SPECIAL_SIZE) as u16).to_le_bytes().as_slice());

        Self { bytes: buff }

    }

    // Header methods

    pub fn get_header(&self) -> &[u8] {
        &self.bytes[0..HEADER_SIZE]
    }

    pub fn get_page_type(&self) -> PageType {
        let byte = self.bytes[PAGE_TYPE_OFFSET];
        PageType::from_byte(byte)
    }

    pub fn slot_dir_ref(&self) -> SlotDir<'_> {

        // TODO This needs to be a method
        let fs_ptr = u16::from_le_bytes(
            self.bytes[FREE_START_OFFSET .. FREE_START_OFFSET + 2].try_into().unwrap()
        ) as usize;

        let sd = self.bytes[HEADER_SIZE..HEADER_SIZE + (HEADER_SIZE - fs_ptr)].as_ref();

        SlotDir { array: sd, }
    }


}

// Slot Array

pub struct SlotDir<'a> {
    array: &'a [u8],
}

// TODO Implement methods on slot dir and iter


struct SlotEntry<'slot_dir> {
    entry: &'slot_dir [u8], // Should be exactly 4 bytes
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_type() {

        let page = SlottedPage::new(123456789, PageType::Internal);
        println!("{:?}", page.get_page_type());

    }

    #[test]
    fn slot_dir() {

        let page = SlottedPage::new(123456789, PageType::Internal);

        let header = page.get_header();

        println!("size of header {:?}", header.len());

        let sd = page.slot_dir_ref();

        println!("{:?}", sd.array.len());

    }


}


