//NOTE: This is the main intersection between disk and in-memory for pages

//NOTE: There are optimisations to be had where we look at all of the pages that we would need for the transaction
// and bitmask or something to get a set on the pages to ensure we are cache and have ready those pages and do not need
// to keep cycling them

//NOTE: We can also further optimise by

use crate::page::PageID;
use crate::page_cache::page_frame::PageFrame;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct BaseFileCache {
    pub cache: Mutex<HashMap<PageID, Arc<PageFrame>>>,
    // Need a transaction table
    // Need a free list
    // Need a next pageID
    // A next TransactionID?
}

impl BaseFileCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }
}
