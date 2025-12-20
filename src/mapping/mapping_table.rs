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
// // Mapping Table
//   ├─ owns latch + residency state
//   ├─ points to frame
//   │
// Buffer Pool
//   ├─ owns frames
//   ├─ decides eviction
//   │
// Buffer Frame
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
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

// For now we return a PageTableHandle because hash tables can move entries around so we need to be able to be sure where our entries are located and not give out references to them as they can move moved so
// that would leave dangling references.
// In different implementations of a hash table we can optimize what we return
// This way we can change PageTableHandle to whatever we want and keep the same API

// For table entry we can use a small atomic state to allow threads to do double-checking for any misses and loading to disk - for this we can use a small enum,
// along with CAS and Ordering

pub(crate) struct PageTableEntry {
    frame: usize,
    state: AtomicU8,
}

pub(crate) type PageTableHandle = Arc<PageTableEntry>;

pub(crate) trait PageTable {
    fn get(&self, page_id: PageID) -> Option<PageTableHandle>;
}

struct NaiveMappingTable {
    map: RwLock<HashMap<PageID, PageTableHandle>>,
}
