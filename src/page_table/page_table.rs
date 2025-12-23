// A mapping table provides a stable indirection layer that maps logical PageIDs
// to their current physical location (e.g. an in-memory page frame or a disk offset).
// It is the authoritative source of page identity, decoupling
// PageIDs from memory residency and layout.
//
// The mapping table is intentionally separated from the buffer pool. The buffer pool
// manages memory residency, eviction, and reuse of page frames, while the mapping table
// ensures that a PageID always resolves to the correct page location, even if the page
// is moved, evicted, or replaced.
//
// This separation allows the buffer pool to evict or relocate pages safely without
// invalidating concurrent readers or writers, and allows higher-level structures
// (e.g. the B-tree) to remain correct under concurrent access.
//
// // Page Table
//   ├─ owns latch + residency state
//   ├─ points to frame
//   │
// Buffer Pool
//   ├─ owns frames
//   ├─ decides eviction
//   │
// Page Frame
//   ├─ owns memory
//   ├─ pin_count + dirty
//   │
// Page
//   ├─ header
//   ├─ slots
//   └─ data
//
// Future implementations can be sharded hash table which will still implement the PageTable trait. Further to this, we can optimize page handle structures
// to be more memory efficient as well as making latches smaller and faster

use crate::page::PageID;
use crate::page_table::page_table_latch::PageTableLatch;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// For now we return a PageTableHandle because hash tables can move entries around so we need to be able to be sure where our entries are located and not give out references to them as they can move moved so
// that would leave dangling references.
// In different implementations of a hash table we can optimize what we return
// This way we can change PageTableHandle to whatever we want and keep the same API

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageTableResult {
    Disk(u64),
    Memory(u64),
}

pub(crate) struct PageTableEntry {
    state: PageTableLatch<PageTableResult>,
}

impl PageTableEntry {
    fn new(id: PageID) -> Self {
        Self {
            state: PageTableLatch::new(PageTableResult::Disk(id.to_offset())),
        }
    }
}

pub(crate) type PageTableHandle = Arc<PageTableEntry>;

pub(crate) trait PageTable {
    fn get(&self, page_id: PageID) -> Option<PageTableHandle>; // We return a handle here so the buffer manager can load from disk and flip the state and change the frame address
    fn insert(&self, page_id: PageID, entry: PageTableHandle);
}

// --------------- Naive Implementation ------------ //

struct NaiveMappingTable {
    map: Arc<RwLock<HashMap<PageID, PageTableHandle>>>,
}

impl NaiveMappingTable {
    pub(crate) fn new() -> Self {
        NaiveMappingTable {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl PageTable for NaiveMappingTable {
    fn get(&self, page_id: PageID) -> Option<PageTableHandle> {
        self.map.read().unwrap().get(&page_id).cloned()
    }
    fn insert(&self, page_id: PageID, entry: PageTableHandle) {
        self.map.write().unwrap().insert(page_id, entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn test_naive_map() {
        let naive = NaiveMappingTable::new();

        let entry_id_1 = PageID(1234);
        let entry_id_2 = PageID(5678);

        let entry1: PageTableHandle = Arc::new(PageTableEntry::new(entry_id_1));
        let entry2: PageTableHandle = Arc::new(PageTableEntry::new(entry_id_2));

        naive.insert(entry_id_1, entry1);
        naive.insert(entry_id_2, entry2);

        // Now we have loaded the entries which are on disk, we need to have threads try to access them

        let thread_count = 10;
        let mut handles = Vec::with_capacity(thread_count);
        let barrier = Arc::new(Barrier::new(thread_count + 1));
        for i in 0..thread_count {
            let b = barrier.clone();
            let n = naive.map.clone();

            let thread = thread::spawn(move || {
                b.wait();
                let first = n.read().unwrap().get(&entry_id_1).unwrap().clone();
                // Now we try to switch to on disk
                if let Ok(result) = first.state.load(|data| {
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    println!("Thread {} is loading with data {:?}", i, data);
                    return Ok(PageTableResult::Memory(entry_id_1.into()));
                }) {
                    println!("Thread {} got final {:?}", i, result);
                }
            });

            handles.push(thread);
        }

        barrier.wait();

        for handle in handles {
            let _ = handle.join();
        }
    }
}
