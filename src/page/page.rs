use std::sync::atomic::AtomicBool;
use crate::page::base_page::InnerPage;
use crate::page::PageID;

pub (crate) struct PageFrame {
    id: PageID,
    //txid?
    dirty: AtomicBool,
    inner_page: InnerPage,
    // more meta data
}

impl PageFrame {
    
    pub fn new() -> Self {
        Self { id: PageID(1), dirty: AtomicBool::new(false), inner_page: InnerPage::new() }
    }
    
}


#[test]
fn print_page() {
    let page = PageFrame::new().inner_page.print_data();
}