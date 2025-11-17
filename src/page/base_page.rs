use std::cell::UnsafeCell;
use std::ops::Deref;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub(crate) type RawPage = [u8; 4096];

// We can use this when we need more than closures, for more complex operations
struct InnerRawReadGuard<'a> {
    latch_read: RwLockReadGuard<'a, ()>,
    data: &'a RawPage,
}

impl <'a> Deref for InnerRawReadGuard<'a> {
    type Target = RawPage;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

// InnerPage is a simple container that holds raw bytes and a latch. It does not know about page types
// or implementations concerned with the raw bytes
pub(crate) struct InnerPage {
    latch: RwLock<()>,
    raw_page: UnsafeCell<RawPage>,
}

impl InnerPage {
    pub fn new() -> Self {
        Self { latch: RwLock::new(()), raw_page: UnsafeCell::new([0u8; 4096]) }
    }

    fn read_guard<'a>(&self) -> InnerRawReadGuard<'_> {
        let guard = self.latch.read().unwrap();
        InnerRawReadGuard{ latch_read: guard, data: unsafe { &*self.raw_page.get() } }
    }

    // pub fn read<P>(&self, f: FnOnce(&RawPage) -> P) -> P {
    //     let guard = self.read_guard();
    //     f(guard.data)
    // }

    pub fn print_data(&self) {
        let read_guard = self.read_guard();
        println!("{:?}", read_guard.data);
    }

}

pub(crate) struct HeapPageRef<'inner> {
    raw: InnerRawReadGuard<'inner>,
}

impl<'inner> HeapPageRef<'inner> {
    pub fn new(page: &'inner InnerPage) -> Self {
        Self { raw: page.read_guard() }
    }

    pub fn fake_id(&self) -> [u8; 2] {
        self.raw[..2].try_into().unwrap()
    }
}

// We need two page types initially, Heap and Index.
// We will need a header, slot array and cell
// The cell will have a header for transactions and row data