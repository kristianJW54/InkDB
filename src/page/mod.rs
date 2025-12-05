use std::ptr;

pub mod raw_page;
pub mod page_frame;
pub mod meta;
pub mod index_page;

pub(crate) type RawPage = [u8; 4096];

#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct PageID(pub u64);

#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct SlotID(pub u16);

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
    u16::from_le_bytes([*ptr, *ptr.add(1)])
}

#[inline]
pub(crate) unsafe fn write_u16_le_unsafe(b_ptr: *mut u8, value: u16) {
    let bytes = value.to_le_bytes();
    ptr::copy_nonoverlapping(bytes.as_ptr(), b_ptr, 2);
}


// ------------- Page Bit Type Masks --------------- //

/*
    bits 7 6 5 4 | 3 2 1 0
    -------------+---------
       subtype   | page_type
*/

pub(super) const PAGE_TYPE_MASK: u8 = 0b0000_1111;
pub(super) const SUBTYPE_MASK:   u8 = 0b1111_0000;


#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PageKind {
    Heap = 0x01,
    Index = 0x02,
    Meta = 0x03,
    Free = 0x04,
    Undefined = 0xFF,
    // Up to 15...
}

impl PageKind {

    pub(super) fn from_u8(pt: u8) -> Option<Self> {
        Some(match pt {
            0x00 => PageKind::Undefined,
            0x01 => PageKind::Heap,
            0x02 => PageKind::Index,
            0x03 => PageKind::Meta,
            0x04 => PageKind::Free,
            _ => return None,
        })
    }
}

// Now we need a PageType new-type which will be able to hold the bits for both page kinds and subtype kinds
// It is constructed by being given

#[derive(Clone, Copy)]
pub(crate) struct PageType(u8);

impl PageType {

    pub(crate) fn new(pt: u8, pst: u8) -> Self {
        let page_type = pt & PAGE_TYPE_MASK;
        let sub_type = (pst & PAGE_TYPE_MASK) << 4;
        Self(page_type | sub_type)
    }

    pub(crate) fn raw(&self) -> u8 {
        self.0
    }

    pub(crate) fn page_type(&self) -> u8 {
        self.0 & PAGE_TYPE_MASK
    }

    pub(crate) fn page_kind(&self) -> PageKind {
        PageKind::from_u8(self.0).unwrap_or_else(|| {
            PageKind::Undefined
        })
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



// TODO - Have mod tests for all files within

#[test]
fn test_bits() {

    println!("page kind = {:?}", PageKind::from_u8(0b000_0001))

}