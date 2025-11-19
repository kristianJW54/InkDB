pub mod base_page;
mod page;

pub(crate) type RawPage = [u8; 4096];

pub(crate) struct PageID(u64);

pub(crate) enum PageType {
    Internal,
    Leaf,
    Meta,
}

//TODO Implement a global page allocator - need to think about what we return
// Do we return RawPage? Or Frame?
