pub mod base_page;
pub mod page;
pub mod meta;


#[inline]
pub(crate) fn read_u16_le(bytes: &[u8]) -> u16 {
    let mut buf = [0u8; 2];
    buf.copy_from_slice(&bytes[..2]);
    u16::from_le_bytes(buf)
}

#[inline]
pub(crate) unsafe fn read_u16_le_unsafe(ptr: *const u8) -> u16 {
    u16::from_le_bytes([*ptr, *ptr.add(1)])
}

pub(crate) type RawPage = [u8; 4096];


#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct PageID(pub u64);

#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct SlotID(pub u16);

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) enum PageType {
    Undefined = 0xFF,
    Internal  = 0x01,
    Leaf      = 0x02,
    Meta      = 0x03,
}

impl PageType {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x01 => PageType::Internal,
            0x02 => PageType::Leaf,
            0x03 => PageType::Meta,
            _ => PageType::Undefined,
        }
    }
}


// TODO - Have mod tests for all files within
