//

use std::sync::atomic::{AtomicU8, Ordering};

enum PageTableLatchState {
    InMemory,
    OnDisk,
    Loading,
    Invalid,
}

const SPIN_LIMIT: u8 = 10;
const YIELD_LIMIT: u8 = 50;

const PT_ON_DISK: u8 = 0;
const PT_LOADING: u8 = 1;
const PT_IN_MEMORY: u8 = 2;
const PT_INVALID: u8 = 3;

pub(crate) struct PageTableLatch<T: Clone> {
    state: AtomicU8,
    data: T,
}

impl<T: Clone> PageTableLatch<T> {
    pub(crate) fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(PT_ON_DISK),
            data,
        }
    }

    pub(crate) fn fetch(&self) -> Result<T, String> {
        // We need to loop and use CAS for one loader many writers - first thread gets the load

        let mut spin_count = 0;

        loop {
            //

            // first check state
            let state = self.state.load(Ordering::Acquire);

            match state {
                // Fast path is we are in-memory and can return
                PT_IN_MEMORY => {
                    println!("found in memory");
                    return Ok(self.data.clone());
                }
                PT_ON_DISK => {
                    println!("found on disk");
                    // We need to use double checking with CAS in order to compete for loading
                    if let Ok(loading) = self.state.compare_exchange(
                        PT_ON_DISK,
                        PT_LOADING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        println!("we are loading");
                        self.state.store(PT_IN_MEMORY, Ordering::Release);
                        return Ok(self.data.clone());
                    } else {
                        println!("someone else is loading");
                        continue;
                    }
                }
                PT_LOADING => {
                    println!("found loading");
                    // spin/wait
                    // fall off into backoff below
                }
                _ => {
                    println!("found invalid");
                    return Err("Invalid state".to_string());
                }
            }

            // ----- Back off policy -------
        }
    }
}
