

// NOTE: The raw slotted page

use std::fmt::{Display, Error, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr;
use crate::page::{read_u16_le, read_u16_le_unsafe, PageID, PageType, RawPage, SlotID};

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
const ENTRY_SIZE: usize = size_of::<SlotEntry>();
const ENTRY_SIZE_U16: u16 = ENTRY_SIZE as u16;

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
const HEADER_SIZE_U16: u16 = HEADER_SIZE as u16;

#[derive(Debug, Clone)]
pub(super) enum CellError {
    EmptySlotDir,
    SlotIDOutOfBounds,
    CorruptCell,
}

impl Display for CellError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CellError::EmptySlotDir => { write!(f, "Empty slot dir") }
            CellError::SlotIDOutOfBounds => { write!(f, "SlotID out of bounds") }
            CellError::CorruptCell => { write!(f, "Corrupt cell") }
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
    fn new(lsn: u64, page_type: PageType, flags: u8, special: u16) -> Self {
        let mut buff = [0u8; 4096];
        let b_ptr = buff.as_mut_ptr();

        buff[0..0+LSN_SIZE].copy_from_slice(lsn.to_le_bytes().as_slice());
        let b = page_type as u8;
        buff[PAGE_TYPE_OFFSET] = b;

        // We must set the offsets to a default
        buff[FREE_START_OFFSET..FREE_START_OFFSET + FREE_START_SIZE].copy_from_slice(HEADER_SIZE_U16.to_le_bytes().as_slice());

        buff[FREE_END_OFFSET..FREE_END_OFFSET + FREE_END_SIZE].copy_from_slice((PAGE_SIZE as u16).to_le_bytes().as_slice());

        // Set flags if needed
        if flags > 0 {
            buff[FLAGS_OFFSET] = flags;
        }

        // Lastly set the special offset at end if provided
        if special > 0 {
            buff[SPECIAL_OFFSET..SPECIAL_OFFSET + SPECIAL_SIZE].copy_from_slice(special.to_le_bytes().as_slice());
        }

        Self { bytes: buff }

    }

    // Header + Meta methods

    #[inline(always)]
    pub(crate) fn get_page_type(&self) -> PageType {
        let byte = self.bytes[PAGE_TYPE_OFFSET];
        PageType::from_byte(byte)
    }

    #[inline(always)]
    fn free_start(&self) -> usize {
        let byte = &self.bytes[FREE_START_OFFSET..FREE_START_OFFSET + FREE_START_SIZE];
        read_u16_le(byte) as usize
    }

    #[inline(always)]
    fn increment_free_start(&mut self, bytes: usize) {
        let cur_fs = self.free_start() as usize;
        let new_fs = cur_fs + bytes;

        debug_assert!(new_fs <= self.free_end() as usize);
        debug_assert!(new_fs >= HEADER_SIZE);

        let fs_u16 = new_fs as u16;

        self.bytes[FREE_START_OFFSET..FREE_START_OFFSET + FREE_START_SIZE]
            .copy_from_slice(&fs_u16.to_le_bytes());

    }

    #[inline(always)]
    fn free_end(&self) -> usize {
        unsafe {
            let ptr = self.bytes.as_ptr().add(FREE_END_OFFSET);
            read_u16_le_unsafe(ptr) as usize
        }
    }

    #[inline(always)]
    fn free_contiguous_space(&self) -> usize {
        self.free_end() - self.free_start()
    }

    #[inline]
    fn free_fragmented_space(&self) -> usize {

        // NOTE: We must iterate slot entries and gather the length of entries which are deleted

        0
    }

    #[inline(always)]
    pub(crate) fn get_special(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(SPECIAL_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(crate) fn get_flags(&self) -> u8 {
        self.bytes[FLAGS_OFFSET]
    }

    // Slot Dir Methods

    pub(super) fn slot_dir_ref(&self) -> SlotRef<'_> {

        let fs = self.free_start();
        assert!(fs >= HEADER_SIZE);
        //SAFETY: This is safe because in order to get the fs_ptr we call the free_start() method on this
        // page which indexing into the bytes of the page returning the offset which is correct and in bounds
        let sd_ptr = unsafe { self.bytes.as_ptr().add(HEADER_SIZE) };

        SlotRef::new(sd_ptr, fs - HEADER_SIZE)

    }

    //NOTE: We have already inserted the row data and done so with the assumption that there is enough space
    // to insert a slot_entry
    //NOTE: Do we need to pass in u16 or if this is called after inserting row data can we pass in ptr?
    fn insert_slot_entry(&mut self, size: u16, offset: u16) -> Result<(), Error> {

        let fs = self.free_start();
        // Get pointer to the start of free space
        let mut ptr = self.bytes.as_mut_ptr().wrapping_add(fs);

        //SAFETY: We know we have valid page space of [u8;4096] this will not fail. However, it is up to the caller
        // for page interpretation and correctness that the space we write is valid free space
        //SAFETY: We call this in a mut self method meaning we have exclusive access to the page
        unsafe {
            let ptr_s = size.to_le_bytes().as_ptr();
            let ptr_o = offset.to_le_bytes().as_ptr();

            ptr::copy_nonoverlapping(ptr_s, ptr, 2);
            ptr::copy_nonoverlapping(ptr_o, ptr.add(2), 2);
        }

        self.increment_free_start(ENTRY_SIZE);

        Ok(())
    }

    // Cell Methods

    //NOTE: We need generic methods which can take a block of bytes and insert them into the free space
    pub(super) fn cell_slice_from_id(&self, slot_id: SlotID) -> Result<&'_ [u8], CellError> {
        // We want to return raw bytes here because we are not concerned with how they are interpreted
        // it is up to the type layers who request the bytes to parse and process.

        let slot_dir = self.slot_dir_ref();
        if slot_dir.size == 0 {
            return Err(CellError::EmptySlotDir);
        }

        let idx = slot_id.0 as usize;
        let index_offset = idx * ENTRY_SIZE;
        if idx > slot_dir.slot_count() {
            return Err(CellError::SlotIDOutOfBounds)
        }

        // TODO Add safety notes and also debug asserts

        unsafe {

            let base = slot_dir.ptr.add(index_offset);

            let offset = read_u16_le_unsafe(base) as usize;
            let length = read_u16_le_unsafe(base.add(2)) as usize;

            let end = offset + length;

            if end > PAGE_SIZE {
                return Err(CellError::CorruptCell)
            }

            return Ok(self.bytes[offset..end].as_ref());

        }
    }

    pub(super) fn cell_slice_from_entry(&self, se: SlotEntry) -> Result<&'_ [u8], Error> {

        // We have a valid slot entry. The only way we would be able to get his is if there also exists a valid
        // cell area

        let offset = se.offset as usize;
        let length = se.length as usize;

        debug_assert!(offset + length < PAGE_SIZE);


        let cell = self.bytes[offset..offset + length].as_ref();
        Ok(cell)
    }

    // Operator Methods

    // Special Section Methods

    fn special_size(&self) -> usize {
        if self[SPECIAL_OFFSET] == 0 {
            return 0;
        }
        PAGE_SIZE - self.bytes[SPECIAL_OFFSET] as usize
    }

    pub(super) fn set_special(&self) -> Option<&'_ [u8]> {
        if self.special_size() == 0 {
            return None
        }
        todo!("finish")
    }



}



// Slot Array

#[derive(Debug)]
pub(super) struct SlotRef<'a> {
    ptr: *const u8, // Ptr to the start of the slot_dir
    size: usize,
    _marker: PhantomData<&'a u8>, // For lifetime
}

// TODO Implement methods on slot dir and iter

impl<'a> SlotRef<'a> {

    // This isn't unsafe yet because we are only storing a raw const pointer and not aliasing or dereferencing
    fn new(start: *const u8, size: usize) -> Self {
        Self { ptr: start, size, _marker: PhantomData }
    }

    fn slot_count(&self) -> usize {
        if self.size == 0 {
            return 0;
        }
        self.size / size_of::<SlotEntry>()
    }

    pub(super) fn iter(&self) -> SlotDirIter<'_> {
        SlotDirIter::new(self.ptr, self.size)
    }


}


#[derive(Debug)]
pub(super) struct SlotEntry {
    length: u16,
    offset: u16,
}

pub(super) struct SlotDirIter<'a> {
    ptr: *const u8,
    size: usize,
    pos: usize,
    _marker: PhantomData<&'a u8>,
}

impl SlotDirIter<'_> {
    fn new(ptr: *const u8, size: usize) -> Self {
        Self { ptr, size, pos: 0, _marker: PhantomData }
    }

    #[inline(always)]
    fn slot_count(&self) -> usize {
        self.size / ENTRY_SIZE
    }

    fn next_entry(&mut self) -> Option<SlotEntry> {

        // We return a SlotEntry because we must take the bytes and give back primitives which we can use
        // to compare and find cells with

        // We need to assert that index is within bounds of slot_dir entries
        if self.pos >= self.slot_count() {
            return None;
        }

        unsafe {
            // TODO Add safety note
            // Start is pointer in the page at the position of the last entry which we advance by ENTRY_SIZE
            let start = self.ptr.add(self.pos * ENTRY_SIZE);

            let length = read_u16_le_unsafe(start);
            let offset = read_u16_le_unsafe(start.add(2));

            self.pos += 1;
            println!("pos = {}", self.pos);

            println!("offset {}, length {}", offset, length);

            Some(SlotEntry { length, offset })
        }
    }
}

impl<'a> Iterator for SlotDirIter<'a> {
    type Item = SlotEntry;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_entry()
    }
}


#[cfg(test)]
mod tests {
    use std::mem;
    use crate::page::index_page::IndexRole;
    use super::*;

    #[test]
    fn page_type() {

        let page = SlottedPage::new(123456789, PageType::Index, 0, 0);
        println!("{:?}", page.get_page_type());

    }

    #[test]
    fn slot_dir() {

        let mut page = SlottedPage::new(123456789, PageType::Index, 0, 0);

        let sd = page.slot_dir_ref();

        println!("slot dir size = {}", sd.size);

        page.insert_slot_entry(100, 12).unwrap();
        page.insert_slot_entry(200, 21).unwrap();
        page.insert_slot_entry(300, 22).unwrap();

        for i in page.slot_dir_ref().iter() {
            println!("{:?}", i);
        }

        let result = page.cell_slice_from_id(SlotID(0)).unwrap();
        println!("result -> {:?}", result.len());

        // TODO Continue test

    }

    #[should_panic]
    #[test]
    fn get_cell_error() {
        let page = SlottedPage::new(123456789, PageType::Index, IndexRole::to_bits(&IndexRole::Internal),0);
        page.cell_slice_from_id(SlotID(0)).unwrap_or_else(|e| panic!("{}", e));
    }


}


