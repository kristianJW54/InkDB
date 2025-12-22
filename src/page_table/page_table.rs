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

// ---------- Naive Implementation ------------ //

struct NaiveMappingTable {
    map: RwLock<HashMap<PageID, PageTableHandle>>,
}

impl NaiveMappingTable {
    pub(crate) fn new() -> Self {
        NaiveMappingTable {
            map: RwLock::new(HashMap::new()),
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

#[test]
fn test_naive_map() {
    let mut naive = NaiveMappingTable::new();

    let entry1: PageTableHandle = Arc::new(PageTableEntry::new(PageID(1234)));
    let entry2: PageTableHandle = Arc::new(PageTableEntry::new(PageID(5678)));

    let (_, d) = entry1.state.peek();
    match d {
        PageTableResult::Disk(_) => {
            println!("We are in disk state")
        }
        PageTableResult::Memory(_) => {
            println!("We are in memory state")
        }
    }
}
