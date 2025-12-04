use std::ptr;

pub mod raw_page;
pub mod page_frame;
pub mod meta;
pub mod index_page;

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

pub(crate) type RawPage = [u8; 4096];


// ------------- Page Bit Type Masks --------------- //

/*
    bits 7 6 5 4 | 3 2 1 0
    -------------+---------
       subtype   | page_type
*/

pub(super) const PAGE_TYPE_MASK: u8 = 0b0000_1111;
pub(super) const SUBTYPE_MASK:   u8 = 0b1111_0000;

pub(super) const PAGE_KIND_HEAP:  u8 = 0;
pub(super) const PAGE_KIND_INDEX: u8 = 1;
pub(super) const PAGE_KIND_META:  u8 = 2;
pub(super) const PAGE_KIND_FREE:  u8 = 3;
// 16 Page types...if needed

pub(super) fn page_type_bits(pt: u8) -> u8 {
    pt & PAGE_TYPE_MASK
}
pub(super) fn page_sub_type_bits(pst: u8) -> u8 {
    // We shift it back here because we have cleared the page_type range and can now 'convert' the high bits
    // back to normal numbers
    (pst & SUBTYPE_MASK) >> 4
}
// TODO Add the create mask function

#[test]
fn test_page_type_bits() {

    let basic_pt: u8 = 200;

    println!("{:08b}", page_type_bits(basic_pt));

}





#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PageType {
    Heap = 0x01,
    Index = 0x02,
    Meta = 0x03,
    Undefined = 0xFF,
}

impl PageType {
    pub(crate) fn from_byte(byte: u8) -> Self {
        match byte {
            0x01 => PageType::Heap,
            0x02 => PageType::Index,
            0x03 => PageType::Meta,
            _ => PageType::Undefined,
        }
    }
}

#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct PageID(pub u64);

#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct SlotID(pub u16);


// TODO - Have mod tests for all files within
