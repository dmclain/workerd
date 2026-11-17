// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

use jsg_test::Harness;

use super::*;

fn default_histogram() -> Histogram {
    Histogram::constructor(1, 9_007_199_254_740_991, 3, 0.0, 0.0).unwrap()
}

#[test]
fn record_converts_safe_integers_exactly() {
    let h = default_histogram();
    h.record(9_007_199_254_740_991.0);
    h.record_big_int(1);
    assert_eq!(h.count_big_int(), 2);
    assert_eq!(h.min_big_int(), 1);
    // The largest value rounds up to its bucket, as in Node.js.
    assert!(h.max_big_int() >= 9_007_199_254_740_991);
    assert_eq!(h.exceeds_big_int(), 0);
}

#[test]
fn record_big_int_out_of_range_counts_as_exceeds() {
    let h = default_histogram();
    h.record_big_int(i64::MAX);
    assert_eq!(h.count_big_int(), 0);
    assert_eq!(h.exceeds_big_int(), 1);
}

#[test]
fn record_delta_converts_milliseconds_to_nanoseconds() {
    let h = default_histogram();
    h.record_delta(1_000.0);
    assert_eq!(h.count_big_int(), 0);
    h.record_delta(1_003.0);
    assert_eq!(h.count_big_int(), 1);
    // 3 ms, within the bucket precision of 3 significant figures.
    let delta = h.min_big_int();
    assert!((2_997_000..=3_000_000).contains(&delta), "{delta}");
}

#[test]
fn record_delta_saturates_far_future_times() {
    let h = default_histogram();
    h.record_delta(1.0);
    // Converts to more nanoseconds than a u64 holds; saturates instead of wrapping.
    h.record_delta(f64::MAX);
    assert_eq!(h.count_big_int(), 0);
    assert_eq!(h.exceeds_big_int(), 1);
}

#[test]
fn threshold_saturates() {
    for threshold in [f64::INFINITY, 1e300, f64::NAN, 2.5] {
        assert!(Histogram::constructor(1, 1000, 3, 10.0, threshold).is_ok());
    }
}

#[test]
fn invalid_options_are_a_range_error() {
    assert!(Histogram::constructor(10, 15, 3, 0.0, 0.0).is_err());
}

#[test]
fn add_to_itself_doubles() {
    let harness = Harness::new();
    harness.run_in_context(|lock, _ctx| {
        let h = jsg::Rc::new(lock, default_histogram());
        for value in [1, 5, 1000] {
            h.get(lock).record_big_int(value);
        }
        let before = h.get(lock).percentiles_big_int().values;
        let other = h.clone(lock);
        h.get(lock).add(lock, other)?;
        assert_eq!(h.get(lock).count_big_int(), 6);
        assert_eq!(
            h.get(lock).percentiles_big_int().values.last(),
            before.last()
        );
        Ok(())
    });
}
