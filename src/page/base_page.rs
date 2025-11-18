use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub(crate) type RawPage = [u8; 4096];

// We can use this when we need more than closures, for more complex operations
#[derive(Debug)]
struct PageReadGuard<'a> {
    latch_read: RwLockReadGuard<'a, RawPage>,
}

impl <'a> Deref for PageReadGuard<'a> {
    type Target = RawPage;
    fn deref(&self) -> &Self::Target {
        &*self.latch_read
    }
}

struct PageWriteGuard<'a> {
    latch_write: RwLockWriteGuard<'a, RawPage>
}

impl <'a> Deref for PageWriteGuard<'a> {
    type Target = RawPage;
    fn deref(&self) -> &Self::Target { &*self.latch_write }
}

impl <'a> DerefMut for PageWriteGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut *self.latch_write }
}

// InnerPage is a simple container that holds raw bytes and a latch. It does not know about page types
// or implementations concerned with the raw bytes
pub(crate) struct InnerPage {
    page: RwLock<RawPage>,
}

impl InnerPage {

    pub fn new() -> Self {
        InnerPage { page: RwLock::new([0; 4096]) }
    }

    fn read_guard(&self) -> PageReadGuard<'_> {
        let guard = self.page.read().unwrap();
        PageReadGuard { latch_read: guard }
    }

    fn write_guard(&self) -> PageWriteGuard<'_> {
        let guard = self.page.write().unwrap();
        PageWriteGuard { latch_write: guard }
    }

    // pub fn read<P>(&self, f: FnOnce(&RawPage) -> P) -> P {
    //     let guard = self.read_guard();
    //     f(guard.data)
    // }

    pub fn print_data(&self) {
        let read_guard = self.read_guard();
        println!("{:?}", read_guard);
    }

}

//------------------------- Page specific types ------------------------------//

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

#[test]
fn two_threads() {

    let page = Arc::new(InnerPage::new());

    let mut threads = Vec::new();

    let p1 = page.clone();
    let thread1 = std::thread::spawn(move || {
        let b = HeapPageRead::from_guard(p1.read_guard()).get_first_byte();
        println!("first byte: {}", b);
    });

    threads.push(thread1);

    let p2 = page.clone();
    let thread2 = std::thread::spawn(move || {

        let w = HeapPageMut::from_guard(p2.write_guard()).write_first_byte(2);
        println!("first byte: {}", HeapPageRead::from_guard(p2.read_guard()).get_first_byte());

        let b = HeapPageRead::from_guard(p2.read_guard()).get_first_byte();
        println!("first byte: {}", b);
    });

    threads.push(thread2);

    for t in threads {
        t.join().unwrap();
    }

}



// We need two page types initially, Heap and Index.
// We will need a header, slot array and cell
// The cell will have a header for transactions and row data