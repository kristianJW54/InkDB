

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
}

impl DerefMut for SlottedPage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

// #[derive(Debug)]
// pub(crate) struct SlottedRead<'guard_lifetime> {
//     bytes: RwLockReadGuard<'guard_lifetime, SlottedPage>,
// }
//
// impl Deref for SlottedRead<'_> {
//     type Target = RawPage;
//     fn deref(&self) -> &Self::Target {
//         &*self.bytes
//     }
// }

