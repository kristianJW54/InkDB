use crate::page::PageID;



// Btree base structure and heavy lifting

pub(crate) struct BtreeHeader {
	version: u64,
	checksum: u64,
	root: PageID,
}