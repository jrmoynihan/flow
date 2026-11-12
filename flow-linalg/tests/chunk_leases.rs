mod support;

use support::{ChunkLeaseLimitExceeded, ChunkLeaseTracker};

#[test]
fn acquisition_counts_simultaneously_live_leases() {
    let tracker = ChunkLeaseTracker::new(2);

    let first = tracker.acquire().expect("first lease");
    assert_eq!(tracker.live(), 1);
    let second = tracker.acquire().expect("second lease");
    assert_eq!(tracker.live(), 2);
    assert_eq!(tracker.peak_live(), 2);

    drop((first, second));
}

#[test]
fn dropping_a_lease_releases_capacity() {
    let tracker = ChunkLeaseTracker::new(1);

    {
        let _lease = tracker.acquire().expect("lease");
        assert_eq!(tracker.live(), 1);
    }

    assert_eq!(tracker.live(), 0);
    let replacement = tracker.acquire().expect("replacement lease");
    assert_eq!(tracker.live(), 1);
    drop(replacement);
    assert_eq!(tracker.live(), 0);
}

#[test]
fn acquisition_over_limit_fails_without_changing_live_count() {
    let tracker = ChunkLeaseTracker::new(1);
    let lease = tracker.acquire().expect("first lease");

    assert_eq!(
        tracker.acquire().expect_err("second lease must fail"),
        ChunkLeaseLimitExceeded {
            maximum: 1,
            attempted: 2,
        }
    );
    assert_eq!(tracker.live(), 1);
    assert_eq!(tracker.peak_live(), 1);

    drop(lease);
}
