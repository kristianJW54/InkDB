// NOTE: The raw slotted page
// Privacy should be mostly super as we only want the page interpreted layers to interact with the slotted_page. This enforces the need for the slotted page to be
// wrapped and does not allow the page to be exposed outside of any page specific wrappers or if it is, we won't be able to do anything with it anyway.

use super::PageStates;
use crate::page::*;
use std::f64::consts::PI;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Range};
use std::ptr::read;
use std::{ptr, slice};

/*

Slotted Page Layout:

SLOTTED PAGE is dumb - it only knows how to make structural changes to the universal base layout

|----------------|--------------|--------------------------------------------|-----------------------|
|     Header     |  Slot Array  |           Cell Region (Free Space)         |      Trailer Space    |
|    24 bytes    |   Variable   |                   Variable                 |        24 bytes       |
|----------------|--------------|--------------------------------------------|-----------------------|
|                |    Entries   |                                            |  Prefix  | Sibling    |
|                |    0..n      |-->Free start                               |  Entry   | Ptrs       |
|                |    4 bytes   |                                 Free end<--|  8 bytes | 8 bytes    |
|----------------|--------------|--------------------------------------------|-----------------------|


*/

//--------------------- Header -------------------------//

// Header is usually 24 bytes long - Looking at Postgres

// -- Log Sequence Number: 8 bytes
// -- Checksum: 2 bytes
// -- Page Type: 1 byte
// -- Flag bit: 1 byte
// -- Free_start: 2 bytes
// -- Free_end  : 2 bytes
// -- Spare Space: 2 bytes // FIXME: Later we can use this - I'm thinking we expand flags to u16 and have a spare u8 or even we expand page type to u16
// -- Fragmented_space: 2 bytes
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
const PREFIX_OFFSET: usize = FREE_END_OFFSET + FREE_END_SIZE; // Prefix special slot entry - NOT sibling pointer space NOTE: Sibling pointer space is now always included
const PREFIX_OFFSET_SIZE: usize = 2;
const FRAG_OFFSET: usize = PREFIX_OFFSET + PREFIX_OFFSET_SIZE;
const FRAG_SIZE: usize = 2;
const TXID_OFFSET: usize = FRAG_OFFSET + FRAG_SIZE;
const TXID_SIZE: usize = 4;

pub(crate) const HEADER_SIZE: usize = TXID_OFFSET + TXID_SIZE;
pub(crate) const HEADER_SIZE_U16: u16 = HEADER_SIZE as u16;

pub(crate) const TRAILER_SIZE: usize = 24;
pub(crate) const TRAILER_SIZE_U16: u16 = TRAILER_SIZE as u16;
pub(crate) const TRAILER_OFFSET: usize = PAGE_SIZE - TRAILER_SIZE;
pub(crate) const TRAILER_OFFSET_U16: u16 = TRAILER_OFFSET as u16;

pub(super) const SIBLING_SPECIAL_SIZE: usize = 16;
pub(super) const SIBLING_SPECIAL_SIZE_U16: u16 = SIBLING_SPECIAL_SIZE as u16;
pub(super) const RIGHT_SIBLING_OFFSET: usize = 8;

pub(crate) const PREFIX_SIZE: usize = size_of::<SlotEntry>();
pub(crate) const PREFIX_SIZE_U16: u16 = PREFIX_OFFSET_SIZE as u16;

pub(crate) type Result<T> = std::result::Result<T, PageError>;

#[derive(Debug, Clone)]
pub(crate) enum PageError {
    EmptySlotDir,
    SlotIDOutOfBounds(u16, u16),
    CorruptCell,
    SpecialOffsetIsZero,
    SlotIndexNotInRange,
    NoContigiousSpace,
    NotEnoughFreeSpace,
    InvalidFreeEnd,
    InvalidFreeStart,
    TransferError,
    InvalidSlotIndex,
    SpecialSpaceCannotBeSet,
    OffsetOutOfBounds(u16, u16),
    InsertError(InsertErrorCtx),
}

#[derive(Debug, Clone)]
pub(crate) struct InsertErrorCtx {
    contiguous_space: u16,
    fragment_space: u16,
    required_space: u16,
}

impl InsertErrorCtx {
    pub(crate) fn new(contiguous_space: u16, fragment_space: u16, required_space: u16) -> Self {
        Self {
            contiguous_space,
            fragment_space,
            required_space,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SlottedPageMut<'a> {
    bytes: &'a mut RawPage,
}

impl<'a> SlottedPageMut<'a> {
    pub(crate) fn from_bytes(bytes: &'a mut RawPage) -> Self {
        Self { bytes: bytes }
    }

    // TODO: Can we remove duplicate ref methods in mut now?
    pub(crate) fn as_ref(&'a self) -> SlottedPageRef<'a> {
        SlottedPageRef::from_bytes(&*self.bytes)
    }

    pub(super) unsafe fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    pub(super) unsafe fn as_mut_ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }

    pub(crate) fn wipe_page(&mut self) {
        self.bytes.fill(0);
    }

    //NOTE: The new method needs to take parameters from the allocator like lsn, checksum etc
    pub(crate) fn init_new(bytes: &'a mut RawPage, page_type: u8, flags: u8) -> Self {
        let mut sp = SlottedPageMut::from_bytes(bytes);

        // Page type byte - we set as undefined because the page type wrapper that calls this should define this
        // If slotted page is initialised and is undefined then it is an invalid page and cannot be operated on
        sp.bytes[PAGE_TYPE_OFFSET] = PageKind::Undefined as u8;

        // free_start -> slot_dir starts immediately after header
        sp.bytes[FREE_START_OFFSET..FREE_START_OFFSET + FREE_START_SIZE]
            .copy_from_slice(&HEADER_SIZE_U16.to_le_bytes());

        // free_end -> by default = Trailer offset (sibling pointers 16 bytes + prefix entry 4 bytes + extra 4 bytes)
        sp.bytes[FREE_END_OFFSET..FREE_END_OFFSET + FREE_END_SIZE]
            .copy_from_slice(&(TRAILER_OFFSET as u16).to_le_bytes());

        // We should also be wrapped or be called by an interpreted layer so we set page_type from what we are passed
        sp.set_page_type(page_type);
        sp.set_flags(flags);

        Self { bytes: sp.bytes }
    }

    #[inline(always)]
    pub(super) fn get_tx_id(&self) -> u64 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(TXID_OFFSET);
            read_u64_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn set_tx_id(&mut self, tx_id: u64) {
        unsafe {
            let ptr = self.bytes.as_mut_ptr().add(TXID_OFFSET);
            write_u64_le_unsafe(ptr, tx_id);
        }
    }

    #[inline(always)]
    pub(super) fn get_lsn(&self) -> u64 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(LSN_OFFSET);
            read_u64_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn set_lsn(&mut self, lsn: u64) {
        unsafe {
            let ptr = self.bytes.as_mut_ptr().add(LSN_OFFSET);
            write_u64_le_unsafe(ptr, lsn);
        }
    }

    #[inline(always)]
    pub(super) fn get_checksum(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(CHECKSUM_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn set_checksum(&mut self, checksum: u16) {
        unsafe {
            let ptr = self.bytes.as_mut_ptr().add(CHECKSUM_OFFSET);
            write_u16_le_unsafe(ptr, checksum);
        }
    }

    #[inline(always)]
    pub(super) fn set_page_type(&mut self, page_type: u8) {
        self.bytes[PAGE_TYPE_OFFSET] = page_type;
    }

    #[inline(always)]
    pub(super) fn get_page_type(&self) -> u8 {
        self.bytes[PAGE_TYPE_OFFSET]
    }

    #[inline(always)]
    pub(super) fn free_start(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(FREE_START_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn free_end(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(FREE_END_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn set_free_start(&mut self, offset: u16) {
        debug_assert!(offset >= HEADER_SIZE_U16);
        debug_assert!(offset <= PAGE_SIZE_U16);

        unsafe {
            let page_ptr = self.bytes.as_mut_ptr().add(FREE_START_OFFSET);
            write_u16_le_unsafe(page_ptr, offset as u16);
        }
    }

    #[inline(always)]
    pub(super) fn increment_free_start(&mut self, bytes: u16) -> u16 {
        let cur_fs = self.free_start();
        let new_fs = cur_fs + bytes;

        debug_assert!(new_fs <= self.free_end());
        debug_assert!(new_fs >= HEADER_SIZE_U16);

        unsafe {
            let page_ptr = self.bytes.as_mut_ptr().add(FREE_START_OFFSET);
            write_u16_le_unsafe(page_ptr, new_fs as u16);
        }
        new_fs
    }

    #[inline(always)]
    pub(super) fn decrement_free_start(&mut self, bytes: u16) -> u16 {
        let cur_fs = self.free_start() as u16;
        let new_fs = cur_fs - bytes;

        assert!(new_fs >= HEADER_SIZE_U16);

        unsafe {
            let b_ptr = self.bytes.as_mut_ptr().add(FREE_START_OFFSET);
            write_u16_le_unsafe(b_ptr, new_fs as u16);
        }

        new_fs
    }

    #[inline]
    pub(super) fn set_free_end(&mut self, offset: u16) -> Result<()> {
        let offset = offset;

        debug_assert!(offset >= HEADER_SIZE_U16);

        if offset < self.free_start() || offset < HEADER_SIZE_U16 {
            return Err(PageError::InvalidFreeEnd);
        }

        if offset > TRAILER_OFFSET as u16 {
            return Err(PageError::InvalidFreeEnd);
        }

        unsafe {
            let page_ptr = self.bytes.as_mut_ptr().add(FREE_END_OFFSET);
            write_u16_le_unsafe(page_ptr, offset as u16);
        }

        Ok(())
    }

    #[inline(always)]
    pub(super) fn free_contiguous_space(&self) -> u16 {
        self.free_end() - self.free_start()
    }

    pub(super) fn get_fragmented_space(&self) -> u16 {
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(FRAG_OFFSET);
            let frag = read_u16_le_unsafe(b_ptr);
            frag
        }
    }

    pub(super) fn increase_fragmented_space(&mut self, amount: u16) {
        let payload_end = TRAILER_OFFSET as u16;

        assert!(amount < payload_end as u16);

        let frag = self.get_fragmented_space() as u16 + amount;

        assert!(frag < payload_end as u16);

        // SAFETY: We have exclusive access to raw page, we know amount is within bounds of page and desired increase is also within bounds so writing new
        // offset is safe
        unsafe {
            let b_ptr = self.bytes.as_mut_ptr().add(FRAG_OFFSET);
            write_u16_le_unsafe(b_ptr, frag);
        }
    }

    #[inline]
    pub(super) fn free_fragmented_space(&self) -> u16 {
        // Because we store fragmented space in header we can simply fetch and return it

        // SAFETY: We have exclusive access to raw page, we know that fragmented space is within bounds of page so reading it is safe
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(FRAG_OFFSET);
            read_u16_le_unsafe(b_ptr)
        }
    }

    #[inline]
    pub(super) fn memory_used_non_frag(&self) -> u16 {
        let payload_end = TRAILER_OFFSET as u16;

        debug_assert!(payload_end >= HEADER_SIZE_U16);
        debug_assert!(payload_end <= PAGE_SIZE_U16 - SIBLING_SPECIAL_SIZE_U16);

        println!("payload end {:?}", payload_end);

        let payload_capacity = payload_end - HEADER_SIZE_U16;
        let free = self.free_contiguous_space();

        println!("payload capacity {:?}", payload_capacity);
        println!("free {:?}", free);

        debug_assert!(free <= payload_capacity);

        payload_capacity - free
    }

    #[inline]
    pub(super) fn memory_used(&self) -> u16 {
        let payload_end = TRAILER_OFFSET as u16;

        debug_assert!(payload_end >= HEADER_SIZE_U16);
        debug_assert!(payload_end <= PAGE_SIZE_U16 - SIBLING_SPECIAL_SIZE_U16);

        let payload_capacity = payload_end - HEADER_SIZE_U16;
        let space = self.free_contiguous_space() + self.get_fragmented_space();

        debug_assert!(space <= payload_capacity);

        payload_capacity - space
    }

    #[inline(always)]
    pub(super) fn get_slot_count(&self) -> u16 {
        let fs = self.free_start();
        debug_assert!(fs >= HEADER_SIZE_U16);
        (fs - HEADER_SIZE_U16) / ENTRY_SIZE_U16
    }

    #[inline(always)]
    pub(super) fn get_flags(&self) -> u8 {
        self.bytes[FLAGS_OFFSET]
    }

    #[inline]
    pub(super) fn set_flags(&mut self, flags: u8) {
        self.bytes[FLAGS_OFFSET] = flags;
    }

    pub(super) fn slot_dir_ref(&self) -> SlotRef<'_> {
        let fs = self.free_start() as u16;
        assert!(fs >= HEADER_SIZE_U16);
        //SAFETY: This is safe because in order to get the fs_ptr we call the free_start() method on this
        // page which indexing into the bytes of the page returning the offset which is correct and in bounds
        let sd_ptr = unsafe { self.bytes.as_ptr().add(HEADER_SIZE) };

        SlotRef::new(sd_ptr, fs - HEADER_SIZE_U16)
    }

    //NOTE: We have already inserted the row data and done so with the assumption that there is enough space
    // to insert a slot_entry
    fn append_slot_entry(&mut self, entry: SlotEntry) -> Result<()> {
        let fs = self.free_start();
        let end = self.free_end();

        if end - fs < ENTRY_SIZE_U16 {
            return Err(PageError::NotEnoughFreeSpace);
        }

        debug_assert!(fs + ENTRY_SIZE_U16 <= PAGE_SIZE_U16);

        //SAFETY: We know we have valid page space of [u8;4096] this will not fail. However, it is up to the caller
        // for page interpretation and correctness that the space we write is valid free space
        //SAFETY: We call this in a mut self method meaning we have exclusive access to the page
        unsafe {
            // Get pointer to the start of free space
            let ptr = self.bytes.as_mut_ptr().add(fs as usize);

            let offset_bytes = entry.offset.to_le_bytes();
            let length_bytes = entry.length.to_le_bytes();

            ptr::copy_nonoverlapping(offset_bytes.as_ptr(), ptr, 2);
            ptr::copy_nonoverlapping(length_bytes.as_ptr(), ptr.add(2), 2);
        }
        self.increment_free_start(ENTRY_SIZE_U16);
        Ok(())
    }

    // TODO insert_slot_entry_at_index() method
    pub(super) fn insert_slot_entry_at_index(&mut self, idx: u16, entry: SlotEntry) -> Result<()> {
        // we need to first allocate a slot entry size at the start of free space and get the number of slots
        // then we take the slot entries and shift them along by slot_entry_size[4]
        // finally we need to add the slot entry to the start of the slot_dir at HEADER_SIZE

        let old_fs = self.free_start();
        let end = self.free_end();

        if end - old_fs < ENTRY_SIZE_U16 {
            return Err(PageError::NotEnoughFreeSpace);
        }

        let slot_count = (old_fs - HEADER_SIZE_U16) / ENTRY_SIZE_U16;

        if idx > slot_count {
            // TODO: For the error - may want to provide the index and the slot count
            return Err(PageError::SlotIndexNotInRange);
        }

        if idx == 0 {
            return self.prepend_slot_entry(entry);
        }

        if idx == slot_count {
            return self.append_slot_entry(entry);
        }

        let index_offset = (HEADER_SIZE_U16 + (idx * ENTRY_SIZE_U16)) as usize;

        unsafe {
            let b_ptr = self.bytes.as_mut_ptr();
            // Shift the slot dir after the index offset
            ptr::copy(
                b_ptr.add(index_offset),
                b_ptr.add(index_offset + ENTRY_SIZE),
                (slot_count - idx) as usize * ENTRY_SIZE,
            );

            // Now we need to copy in the slot entry

            let offset = entry.offset.to_le_bytes();
            let length = entry.length.to_le_bytes();

            ptr::copy_nonoverlapping(offset.as_ptr(), b_ptr.add(index_offset), 2);
            ptr::copy_nonoverlapping(length.as_ptr(), b_ptr.add(index_offset + 2), 2);

            self.increment_free_start(ENTRY_SIZE_U16);

            Ok(())
        }
    }

    fn prepend_slot_entry(&mut self, entry: SlotEntry) -> Result<()> {
        let fs = self.free_start();
        let end = self.free_end();

        if end - fs < ENTRY_SIZE_U16 {
            return Err(PageError::NotEnoughFreeSpace);
        }

        debug_assert!(fs + ENTRY_SIZE_U16 <= PAGE_SIZE_U16);
        debug_assert!(fs >= HEADER_SIZE_U16);

        // SAFETY: We have checked that there is enough free space for at least one entry slot. The caller should have already inserted the cell
        // We can safely add the entry to the beginning of the slot array after allocating entry space in the array.
        unsafe {
            let b_ptr = self.bytes.as_mut_ptr();

            // Shift the whole slot_dir an entry size to the right
            ptr::copy(
                b_ptr.add(HEADER_SIZE),
                b_ptr.add(HEADER_SIZE + ENTRY_SIZE),
                (fs - HEADER_SIZE_U16) as usize,
            );

            // Get the entry bytes
            let offset_bytes = entry.offset.to_le_bytes();
            let length_bytes = entry.length.to_le_bytes();

            // Now copy in the entry bytes to the beginning of the array
            ptr::copy(offset_bytes.as_ptr(), b_ptr.add(HEADER_SIZE), 2);
            ptr::copy(length_bytes.as_ptr(), b_ptr.add(HEADER_SIZE + 2), 2);

            self.increment_free_start(ENTRY_SIZE_U16);

            Ok(())
        }
    }

    // When we remove slot entries, we must remember that we are saying that the cell bytes they point to are now free but fragmented. They are no longer
    // part a of the page, physically and contiguously they exist, but implicity they do not. So when we try to insert new cells, if we do not have enough contiguous space,
    // we must check fragmented (non addressable) space if we can compact and insert there.

    fn remove_slot_index_range<F>(&mut self, range: Range<u16>, mut f: F) -> Result<()>
    where
        // We will go into a loop - and give the caller a closure to operate inside the loop with a slot_entry and cell_bytes
        F: FnMut(&Self, SlotEntryRef, CellRef) -> Result<()>,
    {
        let slot_count = self.slot_dir_ref().slot_count();
        assert!(range.start < range.end);
        assert!(range.end <= slot_count);

        // We make a frag variable to count the bytes as they are freed
        let mut frag = 0;

        // Once we've made our assertions we can iterate over the range
        for i in range.start..range.end {
            let (se, cell) = self.cell_and_entry_from_index(i)?;
            let length = &se.0[2..2 + 2];
            frag += read_u16_le(length);
            f(self, se, cell)?;
        }
        self.increase_fragmented_space(frag);
        self.remove_slot_array_physical(range)?;
        return Ok(());
    }

    fn remove_slot_array_physical(&mut self, range: Range<u16>) -> Result<()> {
        let slot_count = self.slot_dir_ref().slot_count();
        assert!(range.start < range.end);
        assert!(range.end <= slot_count);

        // How many slots are being removed
        let removed = range.end - range.start;
        // How many slots are at the tail after the range
        let tail = slot_count - range.end;

        // What is the size of the removed bytes - we will use this to increment the fragment size
        let removed_bytes = removed * ENTRY_SIZE_U16;
        // What is the size of the bytes that need to be shifted
        let shift_size = tail * ENTRY_SIZE_U16;

        // We need to get the destination offset of where we want to shift the tail bytes to after the removed bytes
        let dst_offset = range.start * ENTRY_SIZE_U16;
        // What is the source destination offset of the tail bytes that need to be shifted
        let src_offset = range.end * ENTRY_SIZE_U16;

        // Get the header size as isize
        let header_size = HEADER_SIZE as isize;

        let end_of_shifted = (dst_offset + shift_size) + HEADER_SIZE_U16;

        // TODO ADD SAFETY
        unsafe {
            // a, b, c, d, e, f, g,
            // a, b,[c, d, e,]f, g, - Range = 2..5
            // a, b, _, _, _, f, g, - Removed = 12 - Shifted = 8
            // a, b, f, g, _, _, _, - Tail = 12

            // if we have tail bytes to shift then we must copy the src_offset to the dst_offset
            let b_ptr = self.bytes.as_mut_ptr();
            if tail > 0 {
                ptr::copy(
                    b_ptr.offset(header_size + src_offset as isize),
                    b_ptr.offset(header_size + dst_offset as isize),
                    shift_size as usize,
                );
            }

            // Now need to zero out the trailing bytes (delete entries)
            // Write out the trailing bytes to be 0
            ptr::write_bytes(
                b_ptr.offset(header_size + end_of_shifted as isize),
                0,
                shift_size as usize,
            )
        }

        // Decrement the free start by the shift size
        println!("Decrementing free start by {}", removed_bytes);
        self.decrement_free_start(removed_bytes);
        Ok(())
    }

    pub(super) fn get_prefix_entry_ref(&self) -> SlotEntryRef<'_> {
        SlotEntryRef(&self.bytes[TRAILER_OFFSET..TRAILER_OFFSET + PREFIX_SIZE])
    }

    pub(super) fn get_prefix_entry(&self) -> SlotEntry {
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(TRAILER_OFFSET);
            SlotEntry {
                offset: read_u16_le_unsafe(b_ptr),
                length: read_u16_le_unsafe(b_ptr.add(2)),
            }
        }
    }

    pub(super) fn set_prefix_entry(&mut self, entry: SlotEntry) -> Result<()> {
        // This is failable as offset could be beyond the PAGE_SIZE

        if entry.offset > TRAILER_OFFSET_U16 {
            return Err(PageError::OffsetOutOfBounds(entry.offset, PAGE_SIZE_U16));
        }

        // Now we can set it

        unsafe {
            let b_ptr = self.bytes.as_mut_ptr().add(TRAILER_OFFSET);
            write_u16_le_unsafe(b_ptr, entry.offset);
            write_u16_le_unsafe(b_ptr.add(2), entry.length);
        }
        Ok(())
    }

    pub(super) fn has_prefix(&self) -> bool {
        self.bytes[TRAILER_OFFSET + 1] != 0 || self.bytes[TRAILER_OFFSET + 2] != 0
    }

    pub(super) fn check_contiguous_insert(&self, cell: &[u8]) -> Result<()> {
        let contiguous = self.free_contiguous_space();
        let frag = self.get_fragmented_space();

        if (cell.len() + ENTRY_SIZE) > contiguous as usize {
            return Err(PageError::InsertError(InsertErrorCtx {
                contiguous_space: contiguous,
                fragment_space: frag,
                required_space: cell.len() as u16 + ENTRY_SIZE_U16,
            }));
        }

        Ok(())
    }

    pub(super) fn insert_cell(&mut self, cell: &[u8], insert_index: u16) -> Result<()> {
        // Check we have enough free space?
        // We talk only to contigious space here because we can return Err(PageError::NoContigiousSpace)
        // And allow the caller to call back into the raw page methods to either compact or split the page

        let free_start = self.free_start();
        let free_end = self.free_end();

        if (cell.len() + ENTRY_SIZE) > (free_end - free_start) as usize {
            return Err(PageError::InsertError(InsertErrorCtx {
                contiguous_space: self.free_contiguous_space(),
                fragment_space: self.get_fragmented_space(),
                required_space: cell.len() as u16 + ENTRY_SIZE_U16,
            }));
        }

        let cell_start_offset = free_end - cell.len() as u16;

        debug_assert!(cell.len() <= u16::MAX as usize);
        debug_assert!(cell_start_offset <= u16::MAX);

        let entry = SlotEntry {
            offset: cell_start_offset,
            length: cell.len() as u16,
        };

        // We now need to start from free_end and grow upwards by copying in the cell data
        // SAFETY: We are copying from a valid slice to a valid memory location and not overlapping
        unsafe {
            let cell_ptr = self.bytes.as_mut_ptr().add(cell_start_offset as usize);
            ptr::copy_nonoverlapping(cell.as_ptr(), cell_ptr, cell.len());
        }

        // After successful insertion we need to update free_end
        self.set_free_end(cell_start_offset)?;

        // Now we insert the slot_entry
        self.insert_slot_entry_at_index(insert_index, entry)?;

        Ok(())
    }

    pub(super) fn insert_cell_raw(&mut self, cell: &[u8]) -> Result<SlotEntry> {
        // Check we have enough free space?
        // We talk only to contigious space here because we can return Err(PageError::NoContigiousSpace)
        // And allow the caller to call back into the raw page methods to either compact or split the page

        let free_start = self.free_start();
        let free_end = self.free_end();

        if (cell.len() + ENTRY_SIZE) > (free_end - free_start) as usize {
            return Err(PageError::InsertError(InsertErrorCtx {
                contiguous_space: self.free_contiguous_space(),
                fragment_space: self.get_fragmented_space(),
                required_space: cell.len() as u16 + ENTRY_SIZE_U16,
            }));
        }

        let cell_start_offset = free_end - cell.len() as u16;

        assert!(cell.len() <= u16::MAX as usize);
        assert!(cell_start_offset <= u16::MAX);

        let entry = SlotEntry {
            offset: cell_start_offset,
            length: cell.len() as u16,
        };

        // We now need to start from free_end and grow upwards by copying in the cell data
        // SAFETY: We are copying from a valid slice to a valid memory location and not overlapping
        unsafe {
            let cell_ptr = self.bytes.as_mut_ptr().add(cell_start_offset as usize);
            ptr::copy_nonoverlapping(cell.as_ptr(), cell_ptr, cell.len());
        }

        // After successful insertion we need to update free_end
        self.set_free_end(cell_start_offset)?;

        // Now we return the slot_entry
        Ok(entry)
    }

    pub(super) fn insert_cell_append(&mut self, cell: &[u8]) -> Result<()> {
        let slot_count = self.get_slot_count();
        return self.insert_cell(cell, slot_count);
    }

    //NOTE: We need generic methods which can take a block of bytes and insert them into the free space
    pub(super) fn cell_slice_from_id(&self, slot_id: SlotID) -> Result<&'_ [u8]> {
        // We want to return raw bytes here because we are not concerned with how they are interpreted
        // it is up to the type layers who request the bytes to parse and process.

        let slot_dir = self.slot_dir_ref();
        let slot_count = slot_dir.slot_count();
        if slot_count == 0 {
            return Err(PageError::EmptySlotDir);
        }

        let idx = slot_id.0;

        if idx >= slot_count {
            return Err(PageError::SlotIDOutOfBounds(idx, slot_count));
        }

        let index_offset = idx * ENTRY_SIZE_U16;

        // TODO Add safety notes and also debug asserts

        unsafe {
            let base = slot_dir.ptr.add(index_offset as usize);

            let offset = read_u16_le_unsafe(base) as usize;
            let length = read_u16_le_unsafe(base.add(2)) as usize;

            let end = offset + length;

            if end > PAGE_SIZE {
                return Err(PageError::CorruptCell);
            }

            return Ok(self.bytes[offset..end].as_ref());
        }
    }

    fn cell_and_entry_from_index(
        &'a self,
        slot_index: u16,
    ) -> Result<(SlotEntryRef<'a>, CellRef<'a>)> {
        let slot_dir = self.slot_dir_ref();
        let slot_count = slot_dir.slot_count() as usize;

        let idx = slot_index as usize;

        assert!(idx < slot_count);

        let index_offset = idx * ENTRY_SIZE;

        // SAFETY: We know we have a valid slot entry and that it is within bounds
        // We can get a reference to the slot directory and use it's stored ptr to offset to the slot_index
        unsafe {
            let b_ptr = slot_dir.ptr.add(index_offset);

            let offset = read_u16_le_unsafe(b_ptr) as usize;
            let length = read_u16_le_unsafe(b_ptr.add(2)) as usize;

            let end = offset + length;

            if end > PAGE_SIZE {
                return Err(PageError::CorruptCell);
            }

            let cell_ref = self.bytes[offset..offset + length].as_ref();
            let se_ref = self.bytes
                [HEADER_SIZE + index_offset..HEADER_SIZE + index_offset + ENTRY_SIZE]
                .as_ref();

            Ok((SlotEntryRef(se_ref), CellRef(cell_ref)))
        }
    }

    pub(super) fn cell_slice_from_entry(&self, se: SlotEntry) -> &'_ [u8] {
        // We have a valid slot entry. The only way we would be able to get this is if there also exists a valid
        // cell area

        let offset = se.offset as usize;
        let length = se.length as usize;

        debug_assert!(offset + length <= PAGE_SIZE);

        let cell = self.bytes[offset..offset + length].as_ref();
        cell
    }

    // Transfer is a basic byte by byte transfer - it should not be used if compression is enabled
    pub(super) fn transfer(&mut self, slot_index: u16, page: &mut SlottedPageMut) -> Result<()> {
        // First validate the slot range is within the page slot array
        let slot_dir = self.slot_dir_ref();
        let slot_count = slot_dir.slot_count() as u16;

        assert!(slot_index <= slot_count);

        // Now we need to transfer the cells to the given page first to ensure this succeeds before we remove our own bytes
        // First we must validate the passed page is ready to receive the bytes - but we don't try to fix, we only want to make sure we are able to
        // do our job
        assert_eq!(page.memory_used_non_frag(), 0);

        self.remove_slot_index_range(slot_index..slot_count, |_, _, cell| {
            // Invariants:
            // - We know we have a blank page with 0 memory and do not need to make any more assumption, we can stay naive and work on bytes
            // - We know that the slot array is ordered already and can be straight copied
            // - We don't need to know about where we are in the tree or if we need a sibling pointer or high key, the page interpreter above will set these

            // Maintaining slot ordering
            // Current Page:
            // [aaab, aaac, aaad, aaae, aaaf, aaag, aaah]
            //                    ^ transfer from here
            // Blank Page:
            // []
            // Append > [aaae]
            // Append > [aaae, aaaf]
            // Append > [aaae, aaaf, aaag]
            // Append > [aaae, aaaf, aaag, aaah]
            //

            page.insert_cell(cell.0, slot_count)?;

            // If we successfully move the cell to new page, we need to remove the slot entry from current page and by doing this we
            // effectively remove the cell from the current page
            // If we error during iteration we can return the SlotEntry or Index of where we errored for any future retries
            Ok(())
        })?;
        return Ok(());
    }

    fn compact(&mut self) -> Result<RawPage> {
        // First create a scratch buffer that we will memcpy once done
        let mut scratch: RawPage = [0u8; PAGE_SIZE];
        let mut sp = SlottedPageMut::init_new(&mut scratch, self.get_page_type(), 0);
        let slot_count = self.get_slot_count() as u16;

        // The objective is to loop through the slot directory - copy over cells (use transfer?)
        // If any special space we should copy this over also
        // And any header specifics we need [page type, flag bit, transaction id] - generate new checksum?

        // For prefix entry we just need to get the cell - copy it over and set the new offset and length as new prefix entry
        let prefix = self.get_prefix_entry();
        if prefix.offset != 0 {
            let cell = self.cell_slice_from_entry(self.get_prefix_entry());
            // Copy it over
            let entry = sp.insert_cell_raw(cell)?;
            sp.set_prefix_entry(entry);
        }

        for i in 0..slot_count {
            if let Ok((_, cell)) = self.cell_and_entry_from_index(i) {
                sp.insert_cell_append(cell.0)?;
                // Unlike transfer we don't really care about clearing up the current page since we will be swapping it out
            }
        }

        // If we are here then we can assume that our page contents have been copied over so now we must do some housekeeping before swapping the memory
        // - Do we need to generate a new checksum? LSN?

        // Now we want to swap the memory
        // SAFETY: We have exclusive access to the current page - we know that the bytes are valid and of the same size and type so can be swapped
        unsafe {
            let src_ptr = sp.bytes.as_ptr(); // scratch (compacted)
            let dst_ptr = self.bytes.as_mut_ptr(); // self (frame)
            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, PAGE_SIZE);
        }

        Ok(scratch)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlottedPageRef<'a> {
    bytes: &'a RawPage,
}

impl<'a> SlottedPageRef<'a> {
    pub(crate) fn from_bytes(bytes: &'a RawPage) -> Self {
        Self { bytes }
    }

    // -----------------------
    // Unsafe Methods

    #[inline(always)]
    pub(super) unsafe fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    // Header + Meta methods

    #[inline(always)]
    pub(super) fn get_tx_id(&self) -> u64 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(TXID_OFFSET);
            read_u64_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn get_lsn(&self) -> u64 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(LSN_OFFSET);
            read_u64_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn get_checksum(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(CHECKSUM_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn get_page_type(&self) -> u8 {
        self.bytes[PAGE_TYPE_OFFSET]
    }

    #[inline(always)]
    pub(super) fn free_start(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(FREE_START_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn free_end(&self) -> u16 {
        unsafe {
            let ptr = self.bytes.as_ptr().add(FREE_END_OFFSET);
            read_u16_le_unsafe(ptr)
        }
    }

    #[inline(always)]
    pub(super) fn free_contiguous_space(&self) -> u16 {
        self.free_end() - self.free_start()
    }

    pub(super) fn get_fragmented_space(&self) -> u16 {
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(FRAG_OFFSET);
            let frag = read_u16_le_unsafe(b_ptr);
            frag
        }
    }

    #[inline]
    pub(super) fn free_fragmented_space(&self) -> usize {
        // Because we store fragmented space in header we can simply fetch and return it

        // SAFETY: We have exclusive access to raw page, we know that fragmented space is within bounds of page so reading it is safe
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(FRAG_OFFSET);
            read_u16_le_unsafe(b_ptr) as usize
        }
    }

    #[inline]
    pub(super) fn memory_used_non_frag(&self) -> u16 {
        let payload_end = TRAILER_OFFSET_U16;
        debug_assert!(payload_end >= HEADER_SIZE_U16);
        debug_assert!(payload_end <= PAGE_SIZE_U16 - SIBLING_SPECIAL_SIZE_U16);

        let payload_capacity = payload_end - HEADER_SIZE_U16;
        let free = self.free_contiguous_space();

        debug_assert!(free <= payload_capacity);

        payload_capacity - free
    }

    #[inline]
    pub(super) fn memory_used(&self) -> u16 {
        let payload_end = TRAILER_OFFSET_U16;
        debug_assert!(payload_end >= HEADER_SIZE_U16);
        debug_assert!(payload_end <= PAGE_SIZE_U16);

        let payload_capacity = payload_end - HEADER_SIZE_U16;
        let space = self.free_contiguous_space() + self.get_fragmented_space();

        debug_assert!(space <= payload_capacity);

        payload_capacity - space
    }

    #[inline(always)]
    pub(super) fn get_flags(&self) -> u8 {
        self.bytes[FLAGS_OFFSET]
    }

    // Slot Dir Methods

    pub(super) fn slot_dir_ref(&self) -> SlotRef<'_> {
        let fs = self.free_start() as u16;
        assert!(fs >= HEADER_SIZE_U16);
        //SAFETY: This is safe because in order to get the fs_ptr we call the free_start() method on this
        // page which indexing into the bytes of the page returning the offset which is correct and in bounds
        let sd_ptr = unsafe { self.bytes.as_ptr().add(HEADER_SIZE) };

        SlotRef::new(sd_ptr, fs - HEADER_SIZE_U16)
    }

    #[inline(always)]
    pub(super) fn get_slot_count(&self) -> u16 {
        let fs = self.free_start();
        debug_assert!(fs >= HEADER_SIZE_U16);
        (fs - HEADER_SIZE_U16) / ENTRY_SIZE_U16
    }

    // Cell Methods

    //NOTE: We need generic methods which can take a block of bytes and insert them into the free space
    pub(super) fn cell_slice_from_id(&self, slot_id: SlotID) -> Result<&'_ [u8]> {
        // We want to return raw bytes here because we are not concerned with how they are interpreted
        // it is up to the type layers who request the bytes to parse and process.

        let slot_dir = self.slot_dir_ref();
        let slot_count = slot_dir.slot_count() as usize;
        if slot_count == 0 {
            return Err(PageError::EmptySlotDir);
        }

        let idx = slot_id.0 as usize;

        if idx >= slot_count {
            return Err(PageError::SlotIDOutOfBounds(idx as u16, slot_count as u16));
        }

        let index_offset = idx * ENTRY_SIZE;

        // TODO Add safety notes and also debug asserts

        unsafe {
            let base = slot_dir.ptr.add(index_offset);

            let offset = read_u16_le_unsafe(base) as usize;
            let length = read_u16_le_unsafe(base.add(2)) as usize;

            let end = offset + length;

            if end > PAGE_SIZE {
                return Err(PageError::CorruptCell); // TODO: Need context error
            }

            return Ok(self.bytes[offset..end].as_ref());
        }
    }

    pub(super) fn cell_slice_from_entry(&self, se: SlotEntry) -> &'_ [u8] {
        // We have a valid slot entry. The only way we would be able to get this is if there also exists a valid
        // cell area

        let offset = se.offset as usize;
        let length = se.length as usize;

        debug_assert!(offset + length <= PAGE_SIZE);

        let cell = self.bytes[offset..offset + length].as_ref();
        cell
    }

    fn cell_and_entry_from_index(&self, slot_index: u16) -> Result<(SlotEntryRef, CellRef)> {
        let slot_dir = self.slot_dir_ref();
        let slot_count = slot_dir.slot_count() as usize;

        let idx = slot_index as usize;

        assert!(idx < slot_count);

        let index_offset = idx * ENTRY_SIZE;

        // SAFETY: We know we have a valid slot entry and that it is within bounds
        // We can get a reference to the slot directory and use it's stored ptr to offset to the slot_index
        unsafe {
            let b_ptr = slot_dir.ptr.add(index_offset);

            let offset = read_u16_le_unsafe(b_ptr) as usize;
            let length = read_u16_le_unsafe(b_ptr.add(2)) as usize;

            let end = offset + length;

            if end > TRAILER_OFFSET {
                return Err(PageError::CorruptCell);
            }

            let cell_ref = self.bytes[offset..offset + length].as_ref();
            let se_ref = self.bytes[index_offset..index_offset + ENTRY_SIZE].as_ref();

            Ok((SlotEntryRef(se_ref), CellRef(cell_ref)))
        }
    }

    // Operator Methods

    // Special Section Methods

    // TODO: Return a slot entry ref?
    pub(super) fn get_prefix_entry_ref(&self) -> SlotEntryRef<'_> {
        SlotEntryRef(self.bytes[TRAILER_OFFSET..TRAILER_OFFSET + PREFIX_SIZE].as_ref())
    }

    pub(super) fn get_prefix_entry(&self) -> SlotEntry {
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(TRAILER_OFFSET);
            SlotEntry {
                offset: read_u16_le_unsafe(b_ptr),
                length: read_u16_le_unsafe(b_ptr.add(2)),
            }
        }
    }

    pub(super) fn has_prefix(&self) -> bool {
        self.bytes[TRAILER_OFFSET + 1] != 0 || self.bytes[TRAILER_OFFSET + 2] != 0
    }
    // Cell area methods
}

// Slot Array

#[derive(Debug)]
pub(super) struct SlotRef<'a> {
    ptr: *const u8, // Ptr to the start of the slot_dir
    size: u16,
    _marker: PhantomData<&'a u8>, // For lifetime
}

// TODO Implement methods on slot dir and iter

impl<'a> SlotRef<'a> {
    // This isn't unsafe yet because we are only storing a raw const pointer and not aliasing or dereferencing
    pub(super) fn new(start: *const u8, size: u16) -> Self {
        Self {
            ptr: start,
            size,
            _marker: PhantomData,
        }
    }

    pub(super) fn slot_count(&self) -> u16 {
        if self.size == 0 {
            return 0;
        }
        self.size / size_of::<SlotEntry>() as u16
    }

    pub(super) fn iter(&self) -> SlotDirIter<'a> {
        SlotDirIter::new(self.ptr, self.size)
    }

    pub(super) fn get_slot_entry(&self, idx: SlotID) -> Result<SlotEntry> {
        assert!(idx.0 < HEADER_SIZE_U16);
        assert!(idx.0 <= self.size);

        // Get the entry index
        let offset_index = (idx.0 * ENTRY_SIZE_U16) as usize;
        unsafe {
            let b_ptr = self.ptr.add(offset_index);
            let offset = read_u16_le_unsafe(b_ptr);
            let length = read_u16_le_unsafe(b_ptr.add(2));
            Ok(SlotEntry::new(offset, length))
        }
    }
}

pub(super) struct SlotDirMut<'a> {
    ptr: *mut u8,
    size: usize,
    _marker: PhantomData<&'a u8>,
}

impl<'a> SlotDirMut<'a> {
    pub(super) fn new(start: *mut u8, size: usize) -> Self {
        Self {
            ptr: start,
            size,
            _marker: PhantomData,
        }
    }
}

pub(super) struct SlotDirIter<'a> {
    ptr: *const u8,
    size: u16,
    pos: u16,
    _marker: PhantomData<&'a u8>,
}

impl SlotDirIter<'_> {
    pub(super) fn new(ptr: *const u8, size: u16) -> Self {
        Self {
            ptr,
            size,
            pos: 0,
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    pub(super) fn slot_count(&self) -> u16 {
        self.size / ENTRY_SIZE_U16
    }

    pub(super) fn next_entry(&mut self) -> Option<SlotEntry> {
        // We return a SlotEntry because we must take the bytes and give back primitives which we can use
        // to compare and find cells with

        // We need to assert that index is within bounds of slot_dir entries
        if self.pos >= self.slot_count() {
            return None;
        }

        unsafe {
            // TODO Add safety note
            // Start is pointer in the page at the position of the last entry which we advance by ENTRY_SIZE
            let start = self.ptr.add(self.pos as usize * ENTRY_SIZE);

            let offset = read_u16_le_unsafe(start);
            let length = read_u16_le_unsafe(start.add(2));

            self.pos += 1;

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

#[derive(Debug, Clone, Copy)]
pub(super) struct SlotEntry {
    offset: u16,
    length: u16,
}

impl SlotEntry {
    pub(super) fn new(offset: u16, length: u16) -> Self {
        SlotEntry { offset, length }
    }
}

impl From<&'_ [u8]> for SlotEntry {
    fn from(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), ENTRY_SIZE);
        unsafe {
            let b_ptr = bytes.as_ptr();
            let offset = read_u16_le_unsafe(b_ptr);
            let length = read_u16_le_unsafe(b_ptr.add(2));
            SlotEntry { offset, length }
        }
    }
}

#[derive(Debug)]
pub(super) struct SlotEntryRef<'a>(&'a [u8]);
pub(super) struct CellRef<'a>(&'a [u8]);

#[cfg(test)]
mod tests {
    use super::*;
    use std::{mem, process};

    fn half_full_page() -> Result<RawPage> {
        let mut rp: RawPage = [0u8; 4096];
        let mut sp = SlottedPageMut::init_new(&mut rp, 255, 0);

        let key_len = 10;
        let entry_cost = ENTRY_SIZE + key_len;

        let payload_end = TRAILER_OFFSET;
        let payload_capacity = payload_end - HEADER_SIZE;
        let target_used = payload_capacity / 2;
        let entries_needed = target_used / entry_cost;

        for _ in 0..=entries_needed - 1 {
            let mut key = [1u8; 10];
            key[9] = 2;
            sp.insert_cell(&key, 0)?;
        }
        sp.insert_cell(b"test  test", 0)?;

        Ok(rp)
    }

    #[test]
    fn getters_and_setters() {
        let mut raw_page: RawPage = [0u8; 4096];
        let mut page = SlottedPageMut::init_new(&mut raw_page, 255, 0);

        // First test page type
        let c_page_type = page.get_page_type();
        assert_eq!(c_page_type, 255);
        let new_page_type: u8 = 1;
        page.set_page_type(new_page_type);
        assert_eq!(page.get_page_type(), new_page_type);

        // Second test free start
        let c_free_start = page.free_start();
        assert_eq!(c_free_start, HEADER_SIZE_U16);
        let new_free_start: u16 = 25;
        page.set_free_start(new_free_start);
        assert_eq!(page.free_start(), new_free_start);

        // Thirs test free end
        let c_free_end = page.free_end();
        assert_eq!(c_free_end, PAGE_SIZE_U16);
        let new_free_end = PAGE_SIZE_U16 - 10;
        page.set_free_end(new_free_end);
        assert_eq!(page.free_end(), new_free_end);

        // TODO Finish testing ------------- it's boring but just do it
    }

    #[test]
    fn check_insert_entry_at_index() {
        let mut raw_page: RawPage = [0u8; 4096];
        // We need a mutable view here to initialize the page
        let mut page = SlottedPageMut::init_new(&mut raw_page, 255, 0);
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

        // Test index
        let insert_index: u16 = 2;

        let mut assert_vec = Vec::with_capacity(4);

        page.append_slot_entry(SlotEntry::new(100, 12)).unwrap();
        assert_vec.push((100, 12));
        page.append_slot_entry(SlotEntry::new(150, 15)).unwrap();
        assert_vec.push((150, 15));
        page.append_slot_entry(SlotEntry::new(200, 40)).unwrap();
        assert_vec.push((200, 40));

        assert_eq!(assert_vec[insert_index as usize].0, 200);
        assert_eq!(assert_vec[insert_index as usize].1, 40);

        page.insert_slot_entry_at_index(
            insert_index,
            SlotEntry {
                length: 30,
                offset: 50,
            },
        )
        .unwrap();
        assert_vec.insert(insert_index as usize, (50, 30));

        assert_eq!(assert_vec[insert_index as usize].0, 50);
        assert_eq!(assert_vec[insert_index as usize].1, 30);
    }

    #[test]
    fn adding_cell_append() {
        let mut raw_page: RawPage = [0u8; 4096];
        // We need a mutable view here to initialize the page
        let mut page = SlottedPageMut::init_new(&mut raw_page, 255, 0);

        let cell = "I am a cell".as_bytes();

        match page.insert_cell(cell, 0) {
            Ok(_) => match page.cell_slice_from_id(SlotID(0)) {
                Ok(cell) => {
                    let string = str::from_utf8(cell).unwrap();
                    assert_eq!(string.as_bytes(), cell);
                }
                Err(e) => println!("error"),
            },
            Err(e) => panic!("Error adding cell"),
        }
    }

    #[test]
    fn memory_usage() {
        let mut raw_page: RawPage = [0u8; 4096];
        let mut page = SlottedPageMut::init_new(&mut raw_page, 255, 0);

        let cell = "I am a cell".as_bytes();

        page.insert_cell(cell, 0).ok().unwrap();

        let want_memory_usage: u16 = cell.len() as u16 + 4;

        assert_eq!(page.memory_used_non_frag(), want_memory_usage);
    }

    #[test]
    fn remove_slot_range() {
        let mut raw_page: RawPage = [0u8; 4096];
        let mut page = SlottedPageMut::init_new(&mut raw_page, 255, 0);

        // Append a bunch of slot entries
        page.append_slot_entry(SlotEntry::new(1000, 10))
            .ok()
            .unwrap(); // A
        page.append_slot_entry(SlotEntry::new(1200, 12))
            .ok()
            .unwrap(); // B
        page.append_slot_entry(SlotEntry::new(1400, 14))
            .ok()
            .unwrap(); // C
        page.append_slot_entry(SlotEntry::new(1600, 16))
            .ok()
            .unwrap(); // D
        page.append_slot_entry(SlotEntry::new(1800, 18))
            .ok()
            .unwrap(); // E
        page.append_slot_entry(SlotEntry::new(2000, 20))
            .ok()
            .unwrap(); // F
        page.append_slot_entry(SlotEntry::new(2200, 22))
            .ok()
            .unwrap(); // G

        let fs = page.free_start();

        let slot_count = page.slot_dir_ref().slot_count();
        assert_eq!(slot_count, 7);

        // Now we need to remove a range from the slot array and check both slot_count and free_start

        let result = page.remove_slot_index_range(2..4, |_, _, _| Ok(()));
        match result {
            Ok(()) => {
                let new_count = page.slot_dir_ref().slot_count();
                assert_eq!(new_count, 5);
                let new_fs = fs - (2 * ENTRY_SIZE_U16);
                assert_eq!(new_fs, page.free_start());
            }
            Err(e) => {
                panic!("Error removing slot range: {:?}", e);
            }
        }
    }

    #[test]
    fn retrieving_cells() {
        let page = half_full_page();
        match page {
            Ok(p) => {
                // We have inserted a test message at index 0 of the slot_array test fetching this
                let sp = SlottedPageRef::from_bytes(&p);
                let str = String::from_utf8_lossy(sp.cell_slice_from_id(SlotID(0)).ok().unwrap());
                assert_eq!(str, "test  test");
            }
            Err(e) => {
                panic!("Error creating half-full page: {:?}", e);
            }
        }
        let page2 = half_full_page();
        match page2 {
            Ok(p) => {
                // We have inserted a test message at index 0 of the slot_array test fetching this
                let sp = SlottedPageRef::from_bytes(&p);
                let str = String::from_utf8_lossy(sp.cell_slice_from_id(SlotID(0)).ok().unwrap());
                assert_eq!(str, "test  test");
            }
            Err(e) => {
                panic!("Error creating half-full page: {:?}", e);
            }
        }
    }

    #[test]
    fn transfer_page() {
        // We are inserting test message at the beginning - half full page prepends so this means test messsage will stay at the end of the page
        let mut page = half_full_page().ok().unwrap();
        let mut sp = SlottedPageMut::from_bytes(&mut page);
        let mut page2: RawPage = [0u8; 4096];
        let mut sp2 = SlottedPageMut::init_new(&mut page2, 255, 0);

        // Now we want to call transfer
        // We are only transferring two items over to the new page - the test key will not be in the same space but the slot entry should maintain order
        let result = sp.transfer(144, &mut sp2);
        match result {
            Ok(_) => {
                for (i, se) in sp2.slot_dir_ref().iter().enumerate() {
                    // We assert memory usage
                    assert_eq!(sp2.memory_used_non_frag(), 20 + 2 * ENTRY_SIZE_U16);
                    // We assert that the test key is the last in the iteration
                    if i == 1 {
                        let key = String::from_utf8_lossy(sp2.cell_slice_from_entry(se));
                        assert_eq!(key, "test  test");
                    }
                }
            }
            Err(e) => {
                println!("Transfer failed: {:?}", e);
            }
        }
    }

    #[test]
    fn compact_page() {
        let mut page = half_full_page().ok().unwrap();
        let mut sp = SlottedPageMut::from_bytes(&mut page);

        // Need to delete a slot entry range to create a fragmented space
        // We'll delete ten slot entries from the middle which will be a size of 40
        //

        // Setup checks
        assert_eq!(sp.memory_used(), 2044);
        assert_eq!(sp.get_fragmented_space(), 0);

        let result = sp.remove_slot_index_range(50..60, |_, _, _| Ok(()));
        match result {
            Ok(_) => {
                // Before compact checks
                assert_eq!(sp.memory_used_non_frag(), 2004);
                assert_eq!(sp.get_fragmented_space(), 100);
                if let Ok(_) = sp.compact() {
                    // After compact checks
                    assert_eq!(sp.memory_used_non_frag(), 1904);
                    assert_eq!(sp.get_fragmented_space(), 0);
                }
            }
            Err(e) => {
                println!("Failed {:?}", e);
            }
        }
    }

    // TODO: Test errors
    // TODO: Write test for inserting into a full page to see what the error is
}
