pub mod base_page;
pub mod page;
pub mod meta;

pub(crate) type RawPage = [u8; 4096];


#[derive(Eq, Hash, PartialEq, Debug)]
pub(crate) struct PageID(pub u64);

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) enum PageType {
    Internal,
    Leaf,
    Meta,
}


// TODO - Have mod tests for all files within
