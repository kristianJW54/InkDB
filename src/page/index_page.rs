use crate::page::{
    PAGE_SIZE, PageFlags, PageStates, SlotID,
    index_cell::{IndexCellOwned, IndexCellRef},
    key_view::cmp_search,
    prefix_compression::find_prefix_offset,
    read_u64_le_unsafe,
    slotted_page::{HEADER_SIZE_U16, PREFIX_SIZE, RIGHT_SIBLING_OFFSET, TRAILER_OFFSET},
    write_u64_le_unsafe,
};

use super::{PageID, PageKind, PageType, SlottedPageMut, SlottedPageRef};
use std::cmp::Ordering;
use std::ops::Deref;

pub(super) enum IndexPageError {
    //
    SlottedPageError(super::PageError),
}

impl From<super::PageError> for IndexPageError {
    fn from(err: super::PageError) -> Self {
        Self::SlottedPageError(err)
    }
}

/// IndexPageRef holds the SlottedPageRef which is under the lifetime of the guard, given out by the PageFrame
/// After we perform operations on a IndexPage, the IndexPageRef is dropped and SlottedPageRef is returned back
/// to the guard.
/// IndexPageRef defines read only methods for indexing logic over the Slotted Page layout.
pub(super) struct IndexPageRef<'page> {
    bytes: SlottedPageRef<'page>,
}

impl<'page> IndexPageRef<'page> {
    pub(super) fn from_slotted_page(page: SlottedPageRef<'page>) -> Self {
        Self { bytes: page }
    }

    pub(super) fn get_page_type(&self) -> PageType {
        PageType::from(self.bytes.get_page_type())
    }

    pub(super) fn kind(&self) -> PageKind {
        self.get_page_type().page_kind()
    }

    pub(super) fn flags(&self) -> PageFlags {
        PageFlags::from(self.bytes.get_flags())
    }

    pub(super) fn page_sub_type(&self) -> u8 {
        self.get_page_type().page_sub_type()
    }

    pub(super) fn prefix_compressed(&self) -> bool {
        if self.flags().has_flag(PageStates::PrefixCompressed) {
            debug_assert!(self.bytes.has_prefix());
            true
        } else {
            false
        }
    }

    pub(super) fn first_insertion_prefix_compression(&self) -> bool {
        let sc = self.bytes.get_slot_count();
        let compressed = self.prefix_compressed();
        if self.flags().has_flag(PageStates::HighKey) && compressed {
            match sc {
                1 => true,
                _ => false,
            }
        } else if compressed {
            match sc {
                0 => true,
                _ => false,
            }
        } else {
            false
        }
    }

    pub(super) fn get_left_sibling(&self) -> Option<PageID> {
        // SAFETY: The pointer is within the bounds of the page bytes and is aligned for reading a u64.
        unsafe {
            let b_ptr = self.bytes.as_ptr().add(TRAILER_OFFSET + PREFIX_SIZE);
            let id = PageID::from(read_u64_le_unsafe(b_ptr));
            if id.0 == 0 { None } else { Some(id) }
        }
    }

    pub(super) fn get_right_sibling(&self) -> Option<PageID> {
        // SAFETY: The pointer is within the bounds of the page bytes and is aligned for reading a u64.
        unsafe {
            let b_ptr = self
                .bytes
                .as_ptr()
                .add(TRAILER_OFFSET + PREFIX_SIZE + RIGHT_SIBLING_OFFSET);
            let id = PageID::from(read_u64_le_unsafe(b_ptr));
            if id.0 == 0 { None } else { Some(id) }
        }
    }

    pub(super) fn find_insertion_index(&self, key: &[u8]) -> Result<SlotID, IndexPageError> {
        // We need to take into account the presence of a potential high key - if we have one, then
        // we need to iterate from index 1

        let mut skip = 0;

        if self.flags().has_flag(PageStates::HighKey) {
            debug_assert!(self.get_right_sibling().is_some());
            skip = 1;
        }

        for (i, se) in self.bytes.slot_dir_ref().iter().enumerate().skip(skip) {
            let cell = IndexCellRef::from(self.bytes, se);

            // The comparison key is a full key which has been encoded for bytewise comparison
            // therefore we need to get a keyview of the current iteration key and compare it with the search key

            match cmp_search(key, cell.get_key_view()) {
                Ordering::Less => return Ok(SlotID(i as u16)),
                Ordering::Equal => return Ok(SlotID(i as u16)),
                Ordering::Greater => continue,
            }
        }

        Ok(SlotID(self.bytes.get_slot_count() as u16))
    }

    pub(super) fn prepare_cell_for_insertion(
        &self,
        key: &[u8],
        child_ptr: PageID,
    ) -> IndexCellOwned {
        // Prepare a cell for insertion into page
        // Here we define checks such as prefix compression and whether or not we can compress the key
        // Then we create an IndexCellOwned to return

        if self.prefix_compressed() {
            let prefix_key = IndexCellRef::from(self.bytes, self.bytes.get_prefix_entry());
            let offset = find_prefix_offset(key, prefix_key.get_key());

            debug_assert!(offset <= std::u16::MAX as usize);

            let suffix = &key[offset..];
            return IndexCellOwned::new(suffix, offset as u16, child_ptr);
        } else {
            // We cannot compress the key
            return IndexCellOwned::new(key, 0, child_ptr);
        }
    }

    // TODO: Need to implement the get prefix cells and cell handling - think about how we want to interact with IndexCell
}

/// IndexPageMut holds the SlottedPageMut which is under the lifetime of the guard, given out by the PageFrame
/// After we perform operations on a page, IndexPage wrappers are dropped and SlottedPage is returned back
/// to the guard
/// IndexPageMut provides mutable methods for indexing logic and operations over the Slotted Page layout
pub(super) struct IndexPageMut<'page> {
    bytes: SlottedPageMut<'page>,
}

impl<'page> IndexPageMut<'page> {
    pub(super) fn from_slotted_page(
        /* Should we re-borrow mut here? */ page: SlottedPageMut<'page>,
    ) -> Self {
        Self { bytes: page }
    }

    pub(super) fn as_ref<'a>(&'a self) -> IndexPageRef<'a> {
        IndexPageRef::from_slotted_page(self.bytes.as_ref())
    }

    pub(super) fn init_in_place(&mut self, lsn: u64, page_type: PageType, flags: PageFlags) {
        self.bytes.wipe_page();
        self.bytes.set_page_type(page_type.into());
        // Set free start to default HEADER_SIZE
        self.bytes.set_free_start(HEADER_SIZE_U16);
        // Set lsn
        self.bytes.set_lsn(lsn);
        // Set flags
        self.bytes.set_flags(flags.0);
        // We don't need to do anything else here as trailer space is concrete and slot array should be empty
        // We have type and flags which should be sufficient for correctness
    }

    pub(super) fn set_page_type(&mut self, page_type: PageType) {
        self.bytes.set_page_type(page_type.into());
    }

    pub(super) fn get_page_type(&self) -> PageType {
        PageType::from(self.bytes.get_page_type())
    }

    pub(super) fn set_sub_type(&mut self, sub_type: u8) {
        let mut new_pt = self.get_page_type();
        new_pt.set_subtype_page_bits(sub_type);
        self.set_page_type(new_pt.into());
    }

    pub(super) fn set_left_sibling(&mut self, left_sibling: PageID) {
        // SAFETY: We have exclusive access to the slotted page and Rawpage bytes
        // We know the offset to the left sibling in the page which is fixed so we are safe to write to it
        unsafe {
            let b_ptr = self.bytes.as_mut_ptr().add(TRAILER_OFFSET + PREFIX_SIZE);
            write_u64_le_unsafe(b_ptr, left_sibling.into());
        }
    }

    pub(super) fn set_right_sibling(&mut self, right_sibling: PageID) {
        // SAFETY: We have exclusive access to the slotted page and Rawpage bytes
        // We know the offset to the right sibling in the page which is fixed so we are safe to write to it
        unsafe {
            let b_ptr = self
                .bytes
                .as_mut_ptr()
                .add(TRAILER_OFFSET + PREFIX_SIZE + RIGHT_SIBLING_OFFSET);
            write_u64_le_unsafe(b_ptr, right_sibling.into());
        }
    }

    pub(super) fn try_insert(
        &mut self,
        key: &[u8],
        child_ptr: PageID,
    ) -> Result<(), IndexPageError> {
        // We need to know if this is the first insertion and if we should compress the key first
        let is_first = self.as_ref().first_insertion_prefix_compression();

        // For prefix compression - if we are first and should compress then we need to insert the reference key now so the cell
        // key can be compressed and added
        if is_first {
            // We need to also insert a reference key which isn't compressed for the prefix space
            let se = self.bytes.insert_cell_raw(key)?;
            // Now we update the prefix slot entry
            self.bytes.set_prefix_entry(se)?;
        }

        // FIXME: We re-borrow the page - later we should optimise this
        let page_ref = self.as_ref();

        // Before we do any work we can simply check if we can insert the key
        let prepared_cell = page_ref.prepare_cell_for_insertion(key, child_ptr);
        self.bytes.check_contiguous_insert(prepared_cell.deref())?;

        let insert_index = page_ref.find_insertion_index(key)?;

        let ctx = InsertCtx {
            cell: prepared_cell,
            value_ptr: child_ptr.0,
            insert_index: insert_index.0,
        };

        self.insert(ctx)
    }

    fn insert(&mut self, ctx: InsertCtx) -> Result<(), IndexPageError> {
        self.bytes.insert_cell(ctx.cell.deref(), ctx.insert_index)?;
        Ok(())
    }

    // TODO: Continue implementing IndexPageMut - Need to be able to handle prefix compression runtime decisions
}

#[derive(Debug)]
pub(super) struct InsertCtx {
    pub(super) cell: IndexCellOwned,
    pub(super) value_ptr: u64,
    pub(super) insert_index: u16,
}

// TODO: Need to do tests
