//! Reusable instrumentation for flow-linalg integration tests.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Deterministic failure returned when a test exceeds its live chunk budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkLeaseLimitExceeded {
    pub maximum: usize,
    pub attempted: usize,
}

impl Display for ChunkLeaseLimitExceeded {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "attempted to hold {} live chunk leases; configured maximum is {}",
            self.attempted, self.maximum
        )
    }
}

impl Error for ChunkLeaseLimitExceeded {}

/// Test-only counter for simultaneously live chunk leases.
///
/// Acquiring a lease increments the live count. Dropping the returned
/// [`ChunkLease`] decrements it. The atomic counter allows tests to share one
/// tracker across worker threads without changing production chunk APIs.
#[derive(Debug)]
pub struct ChunkLeaseTracker {
    maximum: usize,
    live: AtomicUsize,
    peak_live: AtomicUsize,
}

impl ChunkLeaseTracker {
    /// Creates a tracker with the maximum allowed simultaneously live leases.
    pub const fn new(maximum: usize) -> Self {
        Self {
            maximum,
            live: AtomicUsize::new(0),
            peak_live: AtomicUsize::new(0),
        }
    }

    /// Acquires one lease or reports the attempted count without incrementing.
    pub fn acquire(&self) -> Result<ChunkLease<'_>, ChunkLeaseLimitExceeded> {
        let mut current = self.live.load(Ordering::SeqCst);
        loop {
            let attempted = current.saturating_add(1);
            if current >= self.maximum {
                return Err(ChunkLeaseLimitExceeded {
                    maximum: self.maximum,
                    attempted,
                });
            }
            match self
                .live
                .compare_exchange(current, attempted, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    self.peak_live.fetch_max(attempted, Ordering::SeqCst);
                    return Ok(ChunkLease { tracker: self });
                }
                Err(observed) => current = observed,
            }
        }
    }

    /// Current number of live leases.
    pub fn live(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    /// Highest simultaneously live count observed by this tracker.
    pub fn peak_live(&self) -> usize {
        self.peak_live.load(Ordering::SeqCst)
    }
}

/// RAII token representing one live test chunk.
#[derive(Debug)]
#[must_use = "dropping the lease immediately releases its tracked capacity"]
pub struct ChunkLease<'a> {
    tracker: &'a ChunkLeaseTracker,
}

impl Drop for ChunkLease<'_> {
    fn drop(&mut self) {
        self.tracker.live.fetch_sub(1, Ordering::SeqCst);
    }
}
