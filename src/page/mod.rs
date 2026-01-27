use std::ptr;
pub(crate) mod index_cell;
pub(crate) mod index_page;
pub(crate) mod internal_page;
pub(super) mod key_view;
pub(crate) mod leaf;
mod prefix_compression;
mod slotted_page;
pub(crate) use slotted_page::{
    ENTRY_SIZE, HEADER_SIZE, PAGE_SIZE, PageError, SlottedPageMut, SlottedPageRef,
};

use crate::page::index_cell::IndexCellOwned;

pub(crate) type RawPage = [u8; 4096];

// TODO: May need to implement PageID resolver for pointer address and offset from page id
#[derive(Eq, Hash, PartialEq, Debug, Clone, Copy)]
pub struct PageID(pub u64);

impl PageID {
    pub(crate) fn into(self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub(crate) fn to_offset(&self) -> u64 {
        self.0 * PAGE_SIZE as u64
    }
}

impl From<u64> for PageID {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Eq, Hash, PartialEq, Debug, Clone, Copy)]
pub(crate) struct SlotID(pub u16);

impl From<u16> for SlotID {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

#[inline]
pub(crate) fn read_u16_le(bytes: &[u8]) -> u16 {
    let mut buf = [0u8; 2];
    buf.copy_from_slice(&bytes[..2]);
    u16::from_le_bytes(buf)
}

#[inline]
pub(crate) fn write_u16_le(bytes: &mut [u8], value: u16) {
    bytes[..2].copy_from_slice(&value.to_le_bytes());
}

#[inline]
pub(crate) unsafe fn read_u16_le_unsafe(ptr: *const u8) -> u16 {
    std::ptr::read_unaligned(ptr as *const u16).to_le()
}

#[inline]
pub(crate) unsafe fn write_u16_le_unsafe(b_ptr: *mut u8, value: u16) {
    let bytes = value.to_le_bytes();
    ptr::copy_nonoverlapping(bytes.as_ptr(), b_ptr, 2);
}

#[inline]
pub(crate) unsafe fn read_u64_le_unsafe(ptr: *const u8) -> u64 {
    std::ptr::read_unaligned(ptr as *const u64).to_le()
}

#[inline]
pub(crate) unsafe fn write_u64_le_unsafe(b_ptr: *mut u8, value: u64) {
    let bytes = value.to_le_bytes();
    ptr::copy_nonoverlapping(bytes.as_ptr(), b_ptr, 8);
}

// ------------- Page Bit Type Masks --------------- //

/*
    bits 7 6 5 4 | 3 2 1 0
    -------------+---------
       subtype   | page_type
*/

pub(super) const PAGE_TYPE_MASK: u8 = 0b0000_1111;
pub(super) const SUBTYPE_MASK: u8 = 0b1111_0000;

const PT_UNDEFINED: u8 = 0b000_0000;
const PT_HEAP: u8 = 0b0000_0001;
const PT_INDEX_INTERNAL: u8 = 0b0000_0010;
const PT_INDEX_MINI_LEAF: u8 = 0b0000_0011;
const PT_INDEX_LEAF: u8 = 0b0000_0100;
const PT_META: u8 = 0b0000_0101;
const PT_FREE: u8 = 0b0000_0110;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PageKind {
    Heap = 0x01,
    IndexInternal = 0x02,
    IndexMiniLeaf = 0x03,
    IndexLeaf = 0x04,
    Meta = 0x05,
    Free = 0x06,
    Undefined = 0xFF,
    // Up to 15...
}

impl From<PageKind> for u8 {
    fn from(pt: PageKind) -> Self {
        match pt {
            PageKind::Undefined => PT_UNDEFINED,
            PageKind::Heap => PT_HEAP,
            PageKind::IndexInternal => PT_INDEX_INTERNAL,
            PageKind::IndexMiniLeaf => PT_INDEX_MINI_LEAF,
            PageKind::IndexLeaf => PT_INDEX_LEAF,
            PageKind::Meta => PT_META,
            PageKind::Free => PT_FREE,
            _ => PT_UNDEFINED,
        }
    }
}

impl PageKind {
    pub(super) fn from_u8(pt: u8) -> Option<Self> {
        Some(match pt {
            PT_UNDEFINED => PageKind::Undefined,
            PT_HEAP => PageKind::Heap,
            PT_INDEX_INTERNAL => PageKind::IndexInternal,
            PT_INDEX_MINI_LEAF => PageKind::IndexMiniLeaf,
            PT_INDEX_LEAF => PageKind::IndexLeaf,
            PT_META => PageKind::Meta,
            PT_FREE => PageKind::Free,
            _ => return None,
        })
    }

    pub(crate) fn uses_slotted_page_layout(&self) -> bool {
        match self {
            PageKind::Heap => false,
            PageKind::IndexInternal => true,
            PageKind::IndexMiniLeaf => true,
            PageKind::IndexLeaf => true,
            PageKind::Meta => false,
            PageKind::Free => false,
            PageKind::Undefined => false,
        }
    }
}

// Now we need a PageType new-type which will be able to hold the bits for both page kinds and subtype kinds
// It is constructed by being given

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PageType(u8);

impl PageType {
    pub(crate) fn new(pt: u8, pst: u8) -> Self {
        let page_type = pt & PAGE_TYPE_MASK;
        let sub_type = (pst & PAGE_TYPE_MASK) << 4;
        println!("state {:0b}", (page_type | sub_type));
        Self(page_type | sub_type)
    }

    pub(crate) fn raw(&self) -> u8 {
        self.0
    }

    pub(crate) fn page_type(&self) -> u8 {
        self.0 & PAGE_TYPE_MASK
    }

    pub(crate) fn page_kind(&self) -> PageKind {
        PageKind::from_u8(self.page_type()).unwrap_or_else(|| PageKind::Undefined)
    }

    pub(crate) fn page_sub_type(&self) -> u8 {
        (self.0 & SUBTYPE_MASK) >> 4
    }

    pub(crate) fn set_page_type(&mut self, pt: u8) {
        // Clear current bits from the page type field [0..3] using the reverse of page type mask
        // Extract the page type bits and then merge
        self.0 = (self.0 & !PAGE_TYPE_MASK) | (pt & PAGE_TYPE_MASK)
    }

    pub(crate) fn set_subtype_page_bits(&mut self, pst: u8) {
        // First we take out current bits and clear the subtype field [4..7] using the reverse of subtype mask
        // 0b1111_0000 -> 0b0000_1111
        // We then extract the lower bits of pst from the PAGE_TYPE_MASK before shifting them to the upper bits
        // Finally we merge
        self.0 = (self.0 & !SUBTYPE_MASK) | ((pst & PAGE_TYPE_MASK) << 4)
    }
}

impl From<PageType> for u8 {
    fn from(pt: PageType) -> u8 {
        pt.0
    }
}

impl From<u8> for PageType {
    fn from(raw: u8) -> Self {
        PageType(raw)
    }
}

// ------------- Page Flags Bits --------------- //

// We have fewer options as we do with page types because flags must be able to represent multiple states
// And not just one either or.

const NO_STATE: u8 = 0b000_0000;
const FAST_PARENT: u8 = 0b000_0001;
const DELETED: u8 = 0b000_0010;
const HALF_DELETED: u8 = 0b000_0100;
const INCOMPLETE_SPLIT: u8 = 0b000_1000;
const HIGH_KEY: u8 = 0b001_0000;
const PREFIX_COMPRESSED: u8 = 0b010_0000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PageStates {
    NoState,
    FastParent,
    Deleted,
    HalfDeleted,
    IncompleteSplit,
    HighKey,
    PrefixCompressed, // TODO: Need to think if we need this as we can infer from the PrefixOffset if we are compressed or not
}

impl PageStates {
    pub(crate) fn from_u8(pf: u8) -> Option<Self> {
        Some(match pf {
            FAST_PARENT => Self::FastParent,
            DELETED => Self::Deleted,
            HALF_DELETED => Self::HalfDeleted,
            INCOMPLETE_SPLIT => Self::IncompleteSplit,
            HIGH_KEY => Self::HighKey,
            PREFIX_COMPRESSED => Self::PrefixCompressed,
            _ => return None,
        })
    }

    pub(crate) fn bit(self) -> u8 {
        match self {
            Self::FastParent => FAST_PARENT,
            Self::Deleted => DELETED,
            Self::HalfDeleted => HALF_DELETED,
            Self::IncompleteSplit => INCOMPLETE_SPLIT,
            Self::HighKey => HIGH_KEY,
            Self::PrefixCompressed => PREFIX_COMPRESSED,
            _ => return NO_STATE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PageFlags(u8);

impl PageFlags {
    pub(crate) fn new(pf: PageStates) -> Self {
        Self(pf.bit())
    }

    pub(crate) fn set_flag(&mut self, pf: PageStates) {
        self.0 |= pf.bit()
    }

    pub(crate) fn clear_flag(&mut self, pf: PageStates) {
        self.0 &= !pf.bit()
    }

    pub(crate) fn has_flag(&self, pf: PageStates) -> bool {
        (self.0 & pf.bit()) != 0
    }

    pub(crate) fn extract_all_flags(&self) -> Vec<PageStates> {
        let mut flags = Vec::new();

        if self.has_flag(PageStates::FastParent) {
            flags.push(PageStates::FastParent)
        }
        if self.has_flag(PageStates::Deleted) {
            flags.push(PageStates::Deleted)
        }
        if self.has_flag(PageStates::HalfDeleted) {
            flags.push(PageStates::Deleted)
        }
        if self.has_flag(PageStates::IncompleteSplit) {
            flags.push(PageStates::IncompleteSplit)
        }
        if self.has_flag(PageStates::HighKey) {
            flags.push(PageStates::HighKey)
        }

        flags
    }
}

impl From<u8> for PageFlags {
    fn from(bits: u8) -> Self {
        Self(bits)
    }
}

// Levels for the index page

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndexLevel(pub u8);

impl IndexLevel {
    pub(crate) const MAX: u8 = 15;

    pub(crate) fn new(level: u8) -> Self {
        assert!(level <= Self::MAX, "Max level for bit field is 15");
        Self(level)
    }

    pub(crate) fn into(self) -> u8 {
        self.0
    }
}

impl From<u8> for IndexLevel {
    fn from(value: u8) -> IndexLevel {
        IndexLevel::new(value)
    }
}

#[derive(Debug)]
pub(super) struct InsertCtx {
    pub(super) cell: IndexCellOwned,
    pub(super) value_ptr: u64,
    pub(super) insert_index: u16,
}

// TODO - Have mod tests for all files within

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits() {
        let state_1 = PageStates::FastParent;
        let state_2 = PageStates::HighKey;

        let mut page_flags = PageFlags::new(state_1);
        page_flags.set_flag(state_2);

        println!("page_flags = {:?}", page_flags.extract_all_flags());
    }

    // TODO: Write tests for PageKind and PageType to clear up the API
}
