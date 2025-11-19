

//NOTE: This is the main intersection between disk and in-memory for pages

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::page::{PageID, RawPage};

pub struct BaseFileCache {
    cache: Arc<Mutex<HashMap<PageID, Arc<RawPage>>>>, // Keep this for now TODO - We will need a page frame
}

// TODO Get a page frame, small copy out tuple meta data into TupleStruct go from there