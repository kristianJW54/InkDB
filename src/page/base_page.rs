

// NOTE: The raw slotted page

use std::ops::{Deref, DerefMut};
use std::sync::RwLockReadGuard;
use crate::page::RawPage;

// TODO If SlottedPage gets too chaotic with mutating and reading we can split into SlottedRead & SlottedWrite??

/*
SLOTTED PAGE is dumb - it only knows how to make structural changes to the universal base layout
| header | slot array growing upward | free space | cells growing downward |
*/

#[derive(Debug)]
pub(crate) struct SlottedPage {
    bytes: RawPage,
}

impl Deref for SlottedPage {
    type Target = RawPage;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl DerefMut for SlottedPage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

impl Default for SlottedPage {
    fn default() -> Self { Self { bytes: [0u8; 4096] } }
}

impl SlottedPage {
    pub fn mutate_first_byte(&mut self) {
        self.bytes[0] = 1
    }

    pub fn get_first_byte(&self) -> usize {
        self.bytes[0] as usize
    }

    // Start of real methods

}

//--------------------- Header -------------------------//


// Header is usually 24 bytes long - Looking at Postgres

// -- Log Sequence Number: 8 bytes
// -- Checksum: 2 bytes
// -- Flag bit: 2 bytes
// -- Slot count: 2 bytes
// -- Free_start: 2 bytes
// -- Free_end  : 2 bytes
// -- Special_start: 2 bytes
// -- Size and Version: 2 bytes
// -- TransactionID: 4 bytes (Oldest unpruned XMAX on page)

#[repr(C)]
#[derive(Debug)]
struct PageHeader {
    sp_lsn: u64,
    sp_csm: u16,
    sp_flg: u16,
    sp_sc: u16,
    sp_fs: u16,
    sp_fe: u16,
    sp_ss: u16,
    sp_size_and_version: u16,
    sp_txid: u32,
}


