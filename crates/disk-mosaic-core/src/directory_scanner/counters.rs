use std::ops::Add;
use std::sync::atomic::AtomicU64;

/// Counters of the files found directly in the scanned root.
#[derive(Default)]
pub struct RootCounters {
    pub(crate) file_count: AtomicU64,
    pub(crate) total_size: AtomicU64,
    pub(crate) small_files_count: AtomicU64,
    pub(crate) small_files_size: AtomicU64,
}

#[derive(Default)]
pub(crate) struct CountAndSize {
    pub(crate) count: u64,
    pub(crate) size: u64,
}

impl CountAndSize {
    pub(crate) const fn push(&mut self, file_size: u64) {
        self.count += 1;
        self.size += file_size;
    }
}

impl Add<CountAndSize> for CountAndSize {
    type Output = Self;

    fn add(mut self, rhs: CountAndSize) -> Self::Output {
        self.count += rhs.count;
        self.size += rhs.size;
        self
    }
}
