

// NOTE: The raw slotted page

use std::fmt::{Display, Error, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use crate::page::{read_u16_le, PageID, PageType, RawPage, SlotID};

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
const SLOT_ENTRY_SIZE: usize = 4;

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

#[derive(Debug, Clone)]
enum CellError {
    EmptySlotDir,

}

enum SlotError {
    FailedToInsertSlotEntry, // May need to include reason so add (String)?
}

impl Display for SlotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotError::FailedToInsertSlotEntry => { write!(f, "Failed to insert slot entry") }
        }
    }
}

impl Display for CellError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CellError::EmptySlotDir => { write!(f, "Empty slot dir") }
        }
    }
}

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
    fn new(lsn: u64, page_type: PageType) -> Self {
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

    fn get_page_type(&self) -> PageType {
        let byte = self.bytes[PAGE_TYPE_OFFSET];
        PageType::from_byte(byte)
    }

    fn free_start(&self) -> usize {
        // TODO Convert to helper
        u16::from_le_bytes([
            self.bytes[FREE_START_OFFSET],
            self.bytes[FREE_START_OFFSET + 1]
        ]) as usize
    }

    fn free_end(&self) -> usize {
        u16::from_le_bytes([
            self.bytes[FREE_END_OFFSET],
            self.bytes[FREE_END_OFFSET + 1]
        ]) as usize
    }

    // Slot Dir Methods

    fn slot_dir_ref(&self) -> SlotRef<'_> {

        let fs = self.free_start();
        assert!(fs >= HEADER_SIZE);
        let sd = self.bytes[HEADER_SIZE..HEADER_SIZE + (HEADER_SIZE - fs)].as_ref();

        //SAFETY: This is safe because in order to get the fs_ptr we call the free_start() method on this
        // page which indexing into the bytes of the page returning the offset which is correct and in bounds
        let fs_ptr = unsafe { self.bytes.as_ptr().add(fs) };

        SlotRef::new(fs_ptr, fs - HEADER_SIZE)

    }

    // TODO Finish
    fn insert_slot_entry(&mut self) -> Result<(), SlotError> {


        Ok(())
    }

    // Memory Methods

    //NOTE: We need generic methods which can take a block of bytes and insert them into the free space
    fn get_cell_ref(&self, slot_id: SlotID) -> Result<&'_ [u8], CellError> {
        // We want to return raw bytes here because we are not concerned with how they are interpreted
        // it is up to the type layers who request the bytes to parse and process.

        let slot_dir = self.slot_dir_ref();
        if slot_dir.size - HEADER_SIZE <= 0 {
            return Err(CellError::EmptySlotDir);
        }

        // We need to iterate the slot dir and find the id which will give us the ptr to the offset

        Ok(&[0u8]) // TODO Finish
    }


}

// Slot Array

pub struct SlotRef<'a> {
    start: *const u8, // Ptr to the start of the slot_dir
    size: usize,
    pos: usize,
    _marker: PhantomData<&'a u8>, // For lifetime
}

// TODO Implement methods on slot dir and iter

impl SlotRef<'_> {

    // This isn't unsafe yet because we are only storing a raw const pointer and not aliasing or dereferencing
    fn new(start: *const u8, size: usize) -> Self {
        Self { start, size, pos: 0, _marker: PhantomData }
    }

    fn slot_count(&self) -> usize {
        self.size / size_of::<SlotEntry>()
    }

    // NOTE: Why do we need an index and how can we be more concise
    // fn next_entry(&mut self) -> SlotEntry {
    //
    //
    //
    //
    // }




}


struct SlotEntry {
    offset: u16,
    length: u16,
}



#[cfg(test)]
mod tests {
    use std::mem;
    use super::*;

    #[test]
    fn page_type() {

        let page = SlottedPage::new(123456789, PageType::Internal);
        println!("{:?}", page.get_page_type());

    }

    #[test]
    fn slot_dir() {

        let page = SlottedPage::new(123456789, PageType::Internal);

        let sd = page.slot_dir_ref();

        println!("slot dir size = {}", sd.size);

        println!("size of slot entry = {}", mem::size_of::<SlotEntry>());

        // TODO Continue test

    }

    #[should_panic]
    #[test]
    fn get_cell_error() {
        let page = SlottedPage::new(123456789, PageType::Internal);
        page.get_cell_ref(SlotID(0)).unwrap_or_else(|e| panic!("{}", e));
    }


}


