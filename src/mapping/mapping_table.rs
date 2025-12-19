// A mapping table provides a stable indirection layer that maps logical PageIDs
// to their current physical location (e.g. an in-memory page frame or a disk offset).
// It is the authoritative source of page identity and synchronization, decoupling
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

pub(crate) trait PageTable {
    fn get(&self, page_id: PageID) -> Option<&'a MappingEntry>;
}

pub(crate) struct MappingEntry {
    page_id: PageID, // Can we put the ptr or offset inside the lock?
    _latch: RwLock<()>,
    // later 16 bit packed lock along with 48 but address
}

struct NaiveMappingTable {
    map: RwLock<HashMap<PageID, Arc<MappingEntry>>>,
}
