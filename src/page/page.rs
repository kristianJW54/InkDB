use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicBool;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use crate::page::{PageID, PageType, RawPage};
use crate::page::base_page::{SlottedPage};
// NOTE: Above PageFrame would be something like the b-tree node/tree


pub (crate) struct PageFrame {
    id: PageID,
    page_type: PageType,
    //txid?
    dirty: AtomicBool,
    inner_page: RwLock<SlottedPage>,
    // more meta data
}

impl PageFrame {

    pub(crate) fn new_frame_from_page(id: PageID, page_type: PageType, page: SlottedPage) -> Self {
        Self { id, page_type, dirty: AtomicBool::new(false), inner_page: RwLock::new(SlottedPage::default()), }
    }

    pub(crate) fn page_read_guard<'page>(&self) -> PageReadGuard<'_> {
        PageReadGuard { latch_read: self.inner_page.read().unwrap() }
    }

    pub(crate) fn page_write_guard<'page>(&self) -> PageWriteGuard<'_> {
        PageWriteGuard { latch_write: self.inner_page.write().unwrap() }
    }


}

#[derive(Debug)]
pub(crate) struct PageReadGuard<'a> {
    latch_read: RwLockReadGuard<'a, SlottedPage>,
}

impl <'a> Deref for PageReadGuard<'a> {
    type Target = SlottedPage;
    fn deref(&self) -> &Self::Target {
        &*self.latch_read
    }
}

pub(crate) struct PageWriteGuard<'a> {
    latch_write: RwLockWriteGuard<'a, SlottedPage>
}

impl <'a> Deref for PageWriteGuard<'a> {
    type Target = SlottedPage;
    fn deref(&self) -> &Self::Target { &*self.latch_write }
}

impl <'a> DerefMut for PageWriteGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut *self.latch_write }
}


//------------------------- Page specific types ------------------------------//

// Page types interpret over the slotted page for their type

struct HeapPageRead<'page> {
    data: PageReadGuard<'page>,
}

impl <'page> HeapPageRead<'page> {

    fn from_guard(guard: PageReadGuard<'page>) -> Self {
        Self { data: guard }
    }

    fn get_first_byte(&self) -> usize {
        self.data[0] as usize
    }

}

struct HeapPageMut<'page> {
    data: PageWriteGuard<'page>,
}

impl <'page> HeapPageMut<'page> {
    fn from_guard(guard: PageWriteGuard<'page>) -> Self {
        Self { data: guard }
    }

    fn write_first_byte(&mut self, byte: u8) {
        self.data[0] = byte;
    }
}


//------------------------- Page Owned specific types ------------------------------//

// Page owned types construct and build before being framed

pub(crate) struct PageHeapOwned {
    bytes: SlottedPage,
}

impl PageHeapOwned {
    pub fn new() -> Self {
        Self { bytes: SlottedPage::default() }
    }
}


#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use super::*;

    // Testing PageFrame API

    #[test]
    fn test_read_guard() {

        let frame = PageFrame::new_frame_from_page(PageID(1), PageType::Leaf, SlottedPage::default());

        frame.page_read_guard().latch_read.get_first_byte();
        frame.page_write_guard().latch_write.mutate_first_byte();
        let b = frame.page_read_guard().latch_read.get_first_byte();

        assert_eq!(frame.page_read_guard().latch_read.get_first_byte(), b);

    }

    #[test]
    fn holding_the_write() {

        let mut frame = PageFrame::new_frame_from_page(PageID(1), PageType::Leaf, SlottedPage::default());

        let mut write = frame.page_write_guard();

        write.latch_write.mutate_first_byte();

        // Do write work here

        

        //

        let read = frame.page_read_guard();

        drop(read);


    }

    #[test]
    fn new_frame() {
        let mut allocated_frame = SlottedPage::default();
        allocated_frame[0] = 1;
        let frame = PageFrame::new_frame_from_page(PageID(1), PageType::Leaf, allocated_frame);

        assert_eq!(frame.id, PageID(1));
        assert_eq!(frame.page_type, PageType::Leaf);
    }

    #[test]
    fn get_tuple_header() {
        //
    }

    #[test]
    fn two_threads() {

        let page = Arc::new(PageFrame::new_frame_from_page(PageID(1), PageType::Leaf, SlottedPage::default()));

        let mut threads = Vec::new();

        let p1 = page.clone();
        let thread1 = std::thread::spawn(move || {
            let b = HeapPageRead::from_guard(p1.page_read_guard()).get_first_byte();
            println!("first byte: {}", b);
        });

        threads.push(thread1);

        let p2 = page.clone();
        let thread2 = std::thread::spawn(move || {

            let w = HeapPageMut::from_guard(p2.page_write_guard()).write_first_byte(2);
            println!("first byte: {}", HeapPageRead::from_guard(p2.page_read_guard()).get_first_byte());

            let b = HeapPageRead::from_guard(p2.page_read_guard()).get_first_byte();
            println!("first byte: {}", b);
        });

        threads.push(thread2);

        for t in threads {
            t.join().unwrap();
        }

    }

}