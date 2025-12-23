use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU8, Ordering};

// For table entry we can use a small atomic state to allow threads to do double-checking for any misses and loading to disk,
// along with CAS and Ordering
// https://preshing.com/20130930/double-checked-locking-is-fixed-in-cpp11/

const SPIN_LIMIT: u8 = 10;
const YIELD_LIMIT: u8 = 50;

const PT_ON_DISK: u8 = 0;
const PT_LOADING: u8 = 1;
const PT_IN_MEMORY: u8 = 2;
const PT_INVALID: u8 = 3;

pub(super) struct PageTableLatch<T: Clone> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

unsafe impl<T: Clone> Sync for PageTableLatch<T> {}
unsafe impl<T: Clone> Send for PageTableLatch<T> {}

impl<T: Clone> PageTableLatch<T> {
    pub(crate) fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(PT_ON_DISK),
            data: UnsafeCell::new(data),
        }
    }

    pub(super) fn state(&self) -> u8 {
        let state = self.state.load(Ordering::Acquire);

        match state {
            PT_IN_MEMORY => PT_IN_MEMORY,
            PT_LOADING => PT_LOADING,
            PT_ON_DISK => PT_ON_DISK,
            _ => PT_INVALID,
        }
    }

    // Returns the current latch state (raw)
    pub(super) fn current_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    pub(super) fn peek(&self) -> (u8, T) {
        // Read the state using Ordering::Acquire so we can sync with any loader
        let state = self.state.load(Ordering::Acquire);
        // SAFETY: We are able to read the data because we have loaded the state with the correct ordering
        let data = unsafe { &*self.data.get() }.clone();
        (state, data)
    }

    pub(super) fn load(&self, work: impl FnOnce(T) -> Result<T, String>) -> Result<T, String> {
        // We need to loop and use CAS for one loader many writers - first thread gets the load

        let mut spin_count = 0;

        loop {
            //

            // first check state
            let state = self.state.load(Ordering::Acquire);

            match state {
                // Fast path is we are in-memory and can return
                PT_IN_MEMORY => {
                    let res = unsafe { &*self.data.get() }.clone();
                    return Ok(res);
                }
                PT_ON_DISK => {
                    // We need to use double checking with CAS in order to compete for loading
                    if let Ok(loading) = self.state.compare_exchange(
                        PT_ON_DISK,
                        PT_LOADING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        // Do work
                        let loaded = work(unsafe { &*self.data.get() }.clone())?;
                        // We now need to update the loaded data to self
                        //
                        // SAFETY: We are the only thread that can access this data due to the fact that we won the CAS and the state is now PT_LOADING
                        // We used the correct ordering to ensure that the data is loaded before the state is updated
                        // And after we update the data, we ensure that we store using Ordering::Release to ensure that the data is visible to other threads
                        unsafe { *self.data.get() = loaded.clone() };
                        self.state.store(PT_IN_MEMORY, Ordering::Release);
                        return Ok(loaded);
                    } else {
                        continue;
                    }
                }
                PT_LOADING => {
                    // spin/wait
                    // fall off into backoff below
                }
                _ => {
                    println!("found invalid");
                    return Err("Invalid state".to_string());
                }
            }

            // ----- Back off policy -------

            if spin_count < SPIN_LIMIT {
                std::hint::spin_loop();
            } else if spin_count < YIELD_LIMIT {
                std::thread::yield_now();
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            spin_count += 1;
            continue;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn page_latch_thread_benches() {
        let latch = Arc::new(PageTableLatch {
            data: UnsafeCell::new(10),
            state: AtomicU8::new(PT_IN_MEMORY),
        });

        const THREADS: &[usize] = &[1, 2, 10, 20, 50, 100, 250, 500, 750, 1000];

        for &thread in THREADS {
            // Barrier so all threads start at the same time
            let barrier = Arc::new(Barrier::new(thread + 1));

            latch.state.store(PT_ON_DISK, Ordering::Release);

            // Vec to collect handles
            let mut handles = Vec::with_capacity(thread);

            let now = std::time::Instant::now();

            for _ in 0..thread {
                let latch_clone = latch.clone();
                let thread = barrier.clone();
                handles.push(std::thread::spawn(move || {
                    thread.wait();
                    let thread_start = std::time::Instant::now();
                    latch_clone
                        .load(|_| {
                            std::thread::sleep(std::time::Duration::from_millis(30));
                            return Ok(10);
                        })
                        .ok();
                    thread_start.elapsed()
                }));
            }

            barrier.wait();

            let mut times = Vec::with_capacity(thread);
            for t in handles {
                times.push(t.join().unwrap());
            }

            let elapsed = now.elapsed();

            // Use results
            let mut results = times.iter().map(|t| t.as_nanos()).collect::<Vec<u128>>();
            results.sort();

            let first = results.get(0).cloned().unwrap_or(0);
            let p50 = results.get(results.len() / 2).cloned().unwrap_or(0);
            let p90 = results.get(results.len() * 9 / 10).cloned().unwrap_or(0);
            let p99 = results.get(results.len() * 99 / 100).cloned().unwrap_or(0);
            let last = results.last().cloned().unwrap_or(0);

            println!("==============================");
            println!("threads: {}", thread);
            println!("total time: {:?}, thread time: {:?}", elapsed, last);
            println!("latency (ns) min/p50/p90/p99/max:");
            println!("  {} / {} / {} / {} / {}", first, p50, p90, p99, last);
            println!("==============================");
        }
    }

    // Measure with different strategies, backoff, just spin, no thread sleep etc
}
