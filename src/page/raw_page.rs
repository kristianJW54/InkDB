// NOTE: The raw slotted page

use crate::page::*;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr;

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

pub(crate) const PAGE_SIZE: usize = 4096;
pub(crate) const PAGE_SIZE_U16: u16 = PAGE_SIZE as u16;
pub(crate) const ENTRY_SIZE: usize = size_of::<SlotEntry>();
pub(crate) const ENTRY_SIZE_U16: u16 = ENTRY_SIZE as u16;

const LSN_OFFSET: usize = 0;
const LSN_SIZE: usize = 8;
const CHECKSUM_OFFSET: usize = LSN_OFFSET + LSN_SIZE;
const CHECKSUM_SIZE: usize = 2;
const PAGE_TYPE_OFFSET: usize = CHECKSUM_OFFSET + CHECKSUM_SIZE;
const PAGE_TYPE_SIZE: usize = 1;
const FLAGS_OFFSET: usize = PAGE_TYPE_OFFSET + PAGE_TYPE_SIZE;
const FLAGS_SIZE: usize = 1;
const FREE_START_OFFSET: usize = FLAGS_OFFSET + FLAGS_SIZE;
const FREE_START_SIZE: usize = 2;
const FREE_END_OFFSET: usize = FREE_START_OFFSET + FREE_START_SIZE;
const FREE_END_SIZE: usize = 2;
const SPECIAL_OFFSET: usize = FREE_END_OFFSET + FREE_END_SIZE;
const SPECIAL_SIZE: usize = 2;
const SIZE_VERSION_OFFSET: usize = SPECIAL_OFFSET + SPECIAL_SIZE;
const SIZE_VERSION_SIZE: usize = 2;
const TXID_OFFSET: usize = SIZE_VERSION_OFFSET + SIZE_VERSION_SIZE;
const TXID_SIZE: usize = 4;

pub(crate) const HEADER_SIZE: usize = TXID_OFFSET + TXID_SIZE;
const HEADER_SIZE_U16: u16 = HEADER_SIZE as u16;

pub(crate) type Result<T> = std::result::Result<T, PageError>;

#[derive(Debug, Clone)]
pub(crate) enum PageError {
    EmptySlotDir,
    SlotIDOutOfBounds,
    CorruptCell,
    SpecialOffsetIsZero,
    SlotIndexNotInRange,
    NoContigiousSpace,
    NotEnoughFreeSpace,
    InvalidFreeEnd(u16),
    InvalidFreeStart(u16),
}

impl Display for PageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PageError::EmptySlotDir => {
                write!(f, "Empty slot dir")
            }
            PageError::SlotIDOutOfBounds => {
                write!(f, "SlotID out of bounds")
            }
            PageError::CorruptCell => {
                write!(f, "Corrupt cell")
            }
            PageError::SpecialOffsetIsZero => {
                write!(f, "Special offset is zero")
            }
            PageError::SlotIndexNotInRange => {
                write!(f, "SlotIndex is not in range")
            }
            PageError::NoContigiousSpace => {
                write!(f, "No contigious space")
            }
            PageError::NotEnoughFreeSpace => {
                write!(f, "Not enough free space")
            }
            PageError::InvalidFreeEnd(offset) => {
                write!(f, "Invalid free end offset: {}", offset)
            }
            PageError::InvalidFreeStart(offset) => {
                write!(f, "Invalid free start offset: {}", offset)
            }
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
    fn default() -> Self {
        SlottedPage::new_blank()
    }
}

impl SlottedPage {
    pub fn mutate_first_byte(&mut self) {
        self.bytes[0] = 1
    }

    pub fn print_header(&self) {
        println!("header = {:?}", &self.bytes[0..HEADER_SIZE]);
    }

    // -----------------------

    //NOTE: The new method needs to take parameters from the allocator like lsn, checksum etc
    pub fn new_blank() -> Self {
        let mut buff = [0u8; PAGE_SIZE];

        // Page type byte - we set as undefined because the page type wrapper that calls this should define this
        // If slotted page is initialised and is undefined then it is an invalid page and cannot be operated on
        buff[PAGE_TYPE_OFFSET] = PageKind::Undefined as u8;

        // free_start -> slot_dir starts immediately after header
        buff[FREE_START_OFFSET..FREE_START_OFFSET + FREE_START_SIZE]
            .copy_from_slice(&HEADER_SIZE_U16.to_le_bytes());

        // free_end -> by default = PAGE_SIZE, overwritten later by wrappers
        buff[FREE_END_OFFSET..FREE_END_OFFSET + FREE_END_SIZE]
            .copy_from_slice(&(PAGE_SIZE as u16).to_le_bytes());

        Self { bytes: buff }
    }

    // Header + Meta methods

    #[inline(always)]
    pub(crate) fn get_page_type(&self) -> u8 {
        self.bytes[PAGE_TYPE_OFFSET]
    }

    #[inline(always)]
    pub(crate) fn set_page_type(&mut self, page_type: u8) {
        self.bytes[PAGE_TYPE_OFFSET] = page_type;
    }

    #[inline(always)]
    fn free_start(&self) -> usize {
        unsafe {
            let ptr = self.bytes.as_ptr().add(FREE_START_OFFSET);
            read_u16_le_unsafe(ptr) as usize
        }
    }

    #[inline(always)]
    fn increment_free_start(&mut self, bytes: usize) -> Result<usize> {
        let cur_fs = self.free_start();
        let new_fs = cur_fs + bytes;

        debug_assert!(new_fs <= self.free_end());
        debug_assert!(new_fs >= HEADER_SIZE);

        if new_fs < HEADER_SIZE {
            return Err(PageError::InvalidFreeStart(bytes as u16));
        }

        unsafe {
            let page_ptr = self.bytes.as_mut_ptr().add(FREE_START_OFFSET);
            write_u16_le_unsafe(page_ptr, new_fs as u16);
        }

        Ok(new_fs)
    }

    #[inline(always)]
    fn free_end(&self) -> usize {
        unsafe {
            let ptr = self.bytes.as_ptr().add(FREE_END_OFFSET);
            read_u16_le_unsafe(ptr) as usize
        }
    }

    pub(crate) fn set_free_end(&mut self, offset: u16) -> Result<()> {
        debug_assert!(offset >= self.free_start() as u16);
        debug_assert!(offset >= HEADER_SIZE_U16);

        if offset < self.free_start() as u16 {
            return Err(PageError::InvalidFreeEnd(offset));
        }

        unsafe {
            let page_ptr = self.bytes.as_mut_ptr().add(FREE_END_OFFSET);
            write_u16_le_unsafe(page_ptr, offset);
        }

        Ok(())
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
    pub(crate) fn get_special_offset(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(SPECIAL_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(crate) fn set_special_offset(&mut self, special: u16) {
        assert!(special < PAGE_SIZE_U16);
        let offset = PAGE_SIZE_U16 - special;
        unsafe {
            write_u16_le_unsafe(self.bytes.as_mut_ptr().add(SPECIAL_OFFSET), offset);
        }
    }

    #[inline(always)]
    pub(crate) fn get_flags(&self) -> u8 {
        self.bytes[FLAGS_OFFSET]
    }

    #[inline]
    pub(crate) fn set_flags(&mut self, flags: u8) {
        self.bytes[FLAGS_OFFSET] = flags;
    }

    #[inline(always)]
    pub(crate) fn get_lsn(&self) -> u64 {
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(LSN_OFFSET);
            read_u64_le_unsafe(b_ptr)
        }
    }

    #[inline(always)]
    pub(crate) fn set_lsn(&mut self, lsn: u64) {
        unsafe {
            let b_ptr = self.bytes.as_mut_ptr().add(LSN_OFFSET);
            write_u64_le_unsafe(b_ptr, lsn);
        }
    }

    // Slot Dir Methods

    pub(crate) fn slot_dir_ref(&self) -> SlotRef<'_> {
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
    fn append_slot_entry(&mut self, size: u16, offset: u16) -> Result<()> {
        let fs = self.free_start();
        let end = self.free_end();

        if end - fs < ENTRY_SIZE {
            return Err(PageError::NotEnoughFreeSpace);
        }

        //SAFETY: We know we have valid page space of [u8;4096] this will not fail. However, it is up to the caller
        // for page interpretation and correctness that the space we write is valid free space
        //SAFETY: We call this in a mut self method meaning we have exclusive access to the page
        unsafe {
            // Get pointer to the start of free space
            let ptr = self.bytes.as_mut_ptr().wrapping_add(fs);

            let offset_bytes = offset.to_le_bytes();
            let length_bytes = size.to_le_bytes();

            ptr::copy_nonoverlapping(offset_bytes.as_ptr(), ptr, 2);
            ptr::copy_nonoverlapping(length_bytes.as_ptr(), ptr.add(2), 2);
        }
        self.increment_free_start(ENTRY_SIZE)?;
        Ok(())
    }

    // TODO insert_slot_entry_at_index() method
    fn insert_slot_entry_at_index(&mut self, idx: usize, entry: SlotEntry) -> Result<()> {
        // we need to first allocate a slot entry size at the start of free space and get the number of slots
        // then we take the slot entries and shift them along by slot_entry_size[4]
        // finally we need to add the slot entry to the start of the slot_dir at HEADER_SIZE

        let old_fs = self.free_start();
        let end = self.free_end();

        if end - old_fs < ENTRY_SIZE {
            return Err(PageError::NotEnoughFreeSpace);
        }

        let slot_count = (old_fs - HEADER_SIZE) / ENTRY_SIZE;

        if idx > slot_count {
            return Err(PageError::SlotIndexNotInRange);
        }

        if idx == slot_count {
            return self.append_slot_entry(entry.length, entry.offset);
        }

        let index_offset = HEADER_SIZE + (idx * ENTRY_SIZE);

        // TODO add safety
        unsafe {
            let b_ptr = self.bytes.as_mut_ptr();
            // Shift the slot dir after the index offset
            ptr::copy(
                b_ptr.add(index_offset),
                b_ptr.add(index_offset + ENTRY_SIZE),
                (slot_count - idx) * ENTRY_SIZE,
            );

            // Now we need to copy in the slot entry

            let offset = entry.offset.to_le_bytes();
            let length = entry.length.to_le_bytes();

            ptr::copy_nonoverlapping(offset.as_ptr(), b_ptr.add(index_offset), 2);
            ptr::copy_nonoverlapping(length.as_ptr(), b_ptr.add(index_offset + 2), 2);

            self.increment_free_start(ENTRY_SIZE)?;

            Ok(())
        }
    }

    // Cell Methods

    //NOTE: We need generic methods which can take a block of bytes and insert them into the free space
    pub(crate) fn cell_slice_from_id(&self, slot_id: SlotID) -> Result<&'_ [u8]> {
        // We want to return raw bytes here because we are not concerned with how they are interpreted
        // it is up to the type layers who request the bytes to parse and process.

        let slot_dir = self.slot_dir_ref();
        let slot_count = slot_dir.slot_count();
        if slot_count == 0 {
            return Err(PageError::EmptySlotDir);
        }

        let idx = slot_id.0 as usize;

        if idx >= slot_count {
            return Err(PageError::SlotIDOutOfBounds);
        }

        let index_offset = idx * ENTRY_SIZE;

        // TODO Add safety notes and also debug asserts

        unsafe {
            let base = slot_dir.ptr.add(index_offset);

            let offset = read_u16_le_unsafe(base) as usize;
            let length = read_u16_le_unsafe(base.add(2)) as usize;

            let end = offset + length;

            if end > PAGE_SIZE {
                return Err(PageError::CorruptCell);
            }

            return Ok(self.bytes[offset..end].as_ref());
        }
    }

    pub(crate) fn cell_slice_from_entry(&self, se: SlotEntry) -> &'_ [u8] {
        // We have a valid slot entry. The only way we would be able to get this is if there also exists a valid
        // cell area

        let offset = se.offset as usize;
        let length = se.length as usize;

        debug_assert!(offset + length < PAGE_SIZE);

        let cell = self.bytes[offset..offset + length].as_ref();
        cell
    }

    // Operator Methods

    // Special Section Methods

    #[inline(always)]
    fn special_size(&self) -> usize {
        let offset = self.get_special_offset() as usize;
        if offset == 0 {
            return 0;
        }
        debug_assert!(offset <= PAGE_SIZE);
        PAGE_SIZE - offset
    }

    pub(crate) fn get_special_mut(&mut self) -> Result<&'_ mut [u8]> {
        let s_offset = self.get_special_offset() as usize;
        if s_offset == 0 {
            return Err(PageError::SpecialOffsetIsZero);
        }
        let size = PAGE_SIZE - s_offset;
        assert!(size <= PAGE_SIZE);

        Ok(&mut self.bytes[s_offset..s_offset + size])
    }

    pub(crate) fn get_special_ref(&self) -> Result<&'_ [u8]> {
        let s_offset = self.get_special_offset() as usize;
        if s_offset == 0 {
            return Err(PageError::SpecialOffsetIsZero);
        }
        let size = PAGE_SIZE - s_offset;
        assert!(size <= PAGE_SIZE);

        Ok(&self.bytes[s_offset..s_offset + size])
    }

    // Cell area methods

    pub(crate) fn add_cell_append_slot_entry(&mut self, cell: &[u8]) -> Result<()> {
        // Check we have enough free space?
        // We talk only to contigious space here because we can return Err(PageError::NoContigiousSpace)
        // And allow the caller to call back into the raw page methods to either compact or split the page

        let free_start = self.free_start();
        let free_end = self.free_end();

        if (cell.len() + ENTRY_SIZE) > free_end - free_start {
            return Err(PageError::NoContigiousSpace);
        }

        let cell_start_offset = free_end - cell.len();

        assert!(cell.len() <= u16::MAX as usize);
        assert!(cell_start_offset <= u16::MAX as usize);

        self.append_slot_entry(cell.len() as u16, cell_start_offset as u16)?;

        // We now need to start from free_end and grow upwards by copying in the cell data
        // SAFETY: We are copying from a valid slice to a valid memory location and not overlapping
        unsafe {
            let cell_ptr = self.bytes.as_mut_ptr().add(cell_start_offset);
            ptr::copy_nonoverlapping(cell.as_ptr(), cell_ptr, cell.len());
        }

        // After successful insertion we need to update free_end
        self.set_free_end(cell_start_offset as u16)?;

        Ok(())
    }

    pub(crate) fn add_cell_at_slot_entry_index(&mut self, index: usize, cell: &[u8]) -> Result<()> {
        // We check free contigious space and return Error if there is no space for the caller to handle

        let free_start = self.free_start();
        let free_end = self.free_end();

        if (cell.len() + ENTRY_SIZE) > free_end - free_start {
            return Err(PageError::NoContigiousSpace);
        }

        let cell_start_offset = free_end - cell.len();

        assert!(cell.len() <= u16::MAX as usize);
        assert!(cell_start_offset <= u16::MAX as usize);

        self.insert_slot_entry_at_index(
            index,
            SlotEntry::new(cell_start_offset as u16, cell.len() as u16),
        )?;

        // We now copy cell data into the free space
        // SAFETY: We are copying from a valid slice to a valid memory location and not overlapping, we have checked
        // the bounds of the free space and the cell data size is valid.
        unsafe {
            let cell_ptr = self.bytes.as_mut_ptr().add(cell_start_offset);
            ptr::copy_nonoverlapping(cell.as_ptr(), cell_ptr, cell.len());
        }

        // IMPORTANT! Need to ensure we update free_end to reflect the change in page memory and free_space
        self.set_free_end(cell_start_offset as u16)?;

        Ok(())
    }
}

// Slot Array

#[derive(Debug)]
pub struct SlotRef<'a> {
    ptr: *const u8, // Ptr to the start of the slot_dir
    size: usize,
    _marker: PhantomData<&'a u8>, // For lifetime
}

// TODO Implement methods on slot dir and iter

impl<'a> SlotRef<'a> {
    // This isn't unsafe yet because we are only storing a raw const pointer and not aliasing or dereferencing
    fn new(start: *const u8, size: usize) -> Self {
        Self {
            ptr: start,
            size,
            _marker: PhantomData,
        }
    }

    pub fn slot_count(&self) -> usize {
        if self.size == 0 {
            return 0;
        }
        self.size / size_of::<SlotEntry>()
    }

    pub fn iter(&self) -> SlotDirIter<'_> {
        SlotDirIter::new(self.ptr, self.size)
    }
}

pub struct SlotDirIter<'a> {
    ptr: *const u8,
    size: usize,
    pos: usize,
    _marker: PhantomData<&'a u8>,
}

impl SlotDirIter<'_> {
    fn new(ptr: *const u8, size: usize) -> Self {
        Self {
            ptr,
            size,
            pos: 0,
            _marker: PhantomData,
        }
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

            let offset = read_u16_le_unsafe(start);
            let length = read_u16_le_unsafe(start.add(2));

            self.pos += 1;
            println!("pos = {}", self.pos);

            println!("offset {}, length {}", offset, length);

            Some(SlotEntry { offset, length })
        }
    }
}

impl<'a> Iterator for SlotDirIter<'a> {
    type Item = SlotEntry;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_entry()
    }
}

#[derive(Debug)]
pub struct SlotEntry {
    offset: u16,
    length: u16,
}

impl SlotEntry {
    fn new(offset: u16, length: u16) -> Self {
        SlotEntry { offset, length }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{mem, process};

    #[test]
    fn page_type() {
        let page = SlottedPage::default();
        assert_eq!(page.get_page_type(), 0);
    }

    #[test]
    fn slot_dir() {
        let mut page = SlottedPage::default();

        let sd = page.slot_dir_ref();

        println!("slot dir size = {}", sd.size);

        page.append_slot_entry(100, 12).unwrap();
        page.append_slot_entry(200, 21).unwrap();
        page.append_slot_entry(300, 22).unwrap();

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
        let page = SlottedPage::default();
        page.cell_slice_from_id(SlotID(0))
            .unwrap_or_else(|e| panic!("{}", e));
    }

    #[test]
    fn check_undefined_special() {
        let mut page = SlottedPage::default();
        // Should error here
        match page.get_special_mut() {
            Ok(_) => panic!("Expected an error for undefined special area"),
            Err(e) => println!("Correctly errored: {}", e),
        }
    }

    #[test]
    fn check_insert_entry_at_index() {
        let mut page = SlottedPage::default();
        page.insert_slot_entry_at_index(
            0,
            SlotEntry {
                offset: 20,
                length: 10,
            },
        )
        .unwrap_or_else(|err| {
            panic!("Failed to insert slot entry at index: {:?}", err);
        });

        page.append_slot_entry(12, 100).unwrap();
        page.append_slot_entry(15, 150).unwrap();
        page.append_slot_entry(40, 200).unwrap();

        for i in page.slot_dir_ref().iter() {
            println!("{:?}", i);
        }

        println!();

        page.insert_slot_entry_at_index(
            2,
            SlotEntry {
                length: 30,
                offset: 50,
            },
        )
        .unwrap();

        for i in page.slot_dir_ref().iter() {
            println!("{:?}", i);
        }
    }

    #[test]
    fn adding_cell_append() {
        let mut page = SlottedPage::default();

        let cell = "I am a cell".as_bytes();

        match page.add_cell_append_slot_entry(cell) {
            Ok(_) => {
                println!("Cell added successfully");
                match page.cell_slice_from_id(SlotID(0)) {
                    Ok(cell) => {
                        let string = str::from_utf8(cell).unwrap();
                        println!("cell contents: {}", string);
                    }
                    Err(e) => println!("error {}", e),
                }
            }
            Err(e) => panic!("Error adding cell: {}", e),
        }
    }
}
