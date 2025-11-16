pub mod base_page;
mod page;

pub(crate) struct PageID(u64);

pub(crate) enum PageType {
    Internal,
    Leaf,
    Meta,
}