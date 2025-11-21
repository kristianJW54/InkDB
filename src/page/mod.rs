pub mod base_page;
pub mod page;
pub mod meta;

pub(crate) type RawPage = [u8; 4096];

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct PageID(u64);

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) enum PageType {
    Internal,
    Leaf,
    Meta,
}


// TODO - Have mod tests for all files within
