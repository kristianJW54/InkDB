use crate::page::PageID;

//NOTE: This is a meta page for btree indexes
// Many meta-pages may be stored for different tables and are referenced in the main db meta page

pub(crate) struct BTreeMetaPage {
    version: u64,
    checksum: u64,
    root: PageID,
}