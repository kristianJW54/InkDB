

//NOTE: This is the main intersection between disk and in-memory for pages

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::page::{PageID};
use crate::page::base_page::SlottedPage;
use crate::page::page::PageFrame;

pub struct BaseFileCache {
    pub cache: Mutex<HashMap<PageID, Arc<PageFrame>>>,
}

impl BaseFileCache {
    pub fn new() -> Self {
        Self { cache: Mutex::new(HashMap::new()) }
    }
}
