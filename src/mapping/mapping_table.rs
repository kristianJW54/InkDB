// Mapping table stores the mini_pages and acts as an indirection layer for mini_pages and base_leaf pages on disk

// According to bf-tree implementation, the mapping table maps the pageID to either memory address or disk offset
// since both addresses are 48 bits, we have another 16 bits for storing a RWLock also alongside which gives us an AtomicU64 to use for efficiency

/*
    Why Bf-Tree uses this despite the complexity

    In high-performance databases, the bottleneck is often the CPU Cache Line.

        A standard std::sync::RwLock is a large struct (usually 24–32 bytes).

        If you have a billion pages, and each page has a 32-byte lock, you waste 32GB of RAM just on locks.

        By packing the lock into the unused bits of the pointer, the lock takes 0 extra bytes.

    This allows the Bf-Tree to fit more "mini-pages" in the CPU cache, which is the primary reason
    it achieves the performance gains mentioned in the paper. It trades code complexity (for the developer) for extreme memory efficiency (for the machine).

*/

// A mapping entry is not a page, it is an authorative representation of a logical page either in memory or on disk

// Start with the rw lock bits and then logic

// For simplicity we will use a hash table/map
//
//
//INVARIANT:
// For internal pages, the mapping table stores only location.
// The internal frame’s version lock is the sole logical lock.

// MappingEntry.location may change only while holding
// the internal frame’s exclusive version lock.

// Readers must validate the frame version after dereferencing
// the mapping entry pointer.

pub(crate) trait PageMap {
    fn get(&self, page_id: PageID);
}

struct NaiveMappingTable {
    map: RwLock<HashMap<PageID, MappingEntry>>,
}
