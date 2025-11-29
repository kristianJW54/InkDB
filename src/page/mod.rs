pub mod base_page;
pub mod page;
pub mod meta;

pub(crate) type RawPage = [u8; 4096];


#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct PageID(pub u64);

#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct SlotID(pub u64);

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
