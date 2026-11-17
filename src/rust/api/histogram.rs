// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

//! `node-internal:histogram`, the native half of `node:perf_hooks`' `createHistogram()`.
//!
//! A thin layer over the `histogram` crate that mirrors the `Histogram` handle of Node.js's
//! `internalBinding('performance')`. The TypeScript layer
//! (`src/node/internal/perf_hooks_histogram.ts`) validates the arguments.

#![expect(
    clippy::cast_precision_loss,
    reason = "values are converted to f64 exactly where Node.js converts them to double"
)]

use std::cell::RefCell;

use histogram::Options;
use jsg_macros::jsg_constructor;
use jsg_macros::jsg_method;
use jsg_macros::jsg_resource;
use jsg_macros::jsg_struct;

fn to_jsg_error(error: &histogram::Error) -> jsg::Error {
    jsg::Error::new_range_error(error.to_string())
}

/// The result of `percentilesBigInt()`: the percentiles and their values, in iteration order.
#[jsg_struct]
#[derive(Debug)]
pub struct PercentilesBigInt {
    pub percentiles: Vec<f64>,
    pub values: Vec<i64>,
}

/// A recordable HDR histogram.
#[jsg_resource(custom_trace)]
pub struct Histogram {
    inner: RefCell<histogram::Histogram>,
}

// A histogram holds only counts and statistics.
impl jsg::Traced for Histogram {
    fn trace(&self, _visitor: &mut jsg::GcVisitor) {}
}

#[jsg_resource]
impl Histogram {
    /// `threshold` is any number of at least 0. It is truncated to an integer, saturating, as
    /// Node.js casts it.
    ///
    /// # Errors
    /// A `RangeError` if the options do not describe a valid layout.
    #[jsg_constructor]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "`threshold` truncates and saturates as in Node.js"
    )]
    fn constructor(
        lowest: i64,
        highest: i64,
        figures: i32,
        half_life: f64,
        threshold: f64,
    ) -> jsg::Result<Self> {
        let options = Options {
            lowest,
            highest,
            figures,
            half_life,
            threshold: threshold as i64,
        };
        let inner = histogram::Histogram::new(&options).map_err(|e| to_jsg_error(&e))?;
        Ok(Self {
            inner: RefCell::new(inner),
        })
    }

    #[jsg_method]
    pub fn count(&self) -> f64 {
        self.inner.borrow().count() as f64
    }

    #[jsg_method]
    pub fn count_big_int(&self) -> u64 {
        self.inner.borrow().count()
    }

    #[jsg_method]
    pub fn exceeds(&self) -> f64 {
        self.inner.borrow().exceeds() as f64
    }

    #[jsg_method]
    pub fn exceeds_big_int(&self) -> u64 {
        self.inner.borrow().exceeds()
    }

    #[jsg_method]
    pub fn reset_count(&self) -> f64 {
        self.inner.borrow().reset_count() as f64
    }

    #[jsg_method]
    pub fn min(&self) -> f64 {
        self.inner.borrow().min() as f64
    }

    #[jsg_method]
    pub fn min_big_int(&self) -> i64 {
        self.inner.borrow().min()
    }

    #[jsg_method]
    pub fn max(&self) -> f64 {
        self.inner.borrow().max() as f64
    }

    #[jsg_method]
    pub fn max_big_int(&self) -> i64 {
        self.inner.borrow().max()
    }

    #[jsg_method]
    pub fn mean(&self) -> f64 {
        self.inner.borrow().mean()
    }

    #[jsg_method]
    pub fn stddev(&self) -> f64 {
        self.inner.borrow().stddev()
    }

    #[jsg_method]
    pub fn skewness(&self) -> f64 {
        self.inner.borrow().skewness()
    }

    #[jsg_method]
    pub fn kurtosis(&self) -> f64 {
        self.inner.borrow().kurtosis()
    }

    #[jsg_method]
    pub fn ewma_mean(&self) -> f64 {
        self.inner.borrow().ewma_mean()
    }

    #[jsg_method]
    pub fn ewma_stddev(&self) -> f64 {
        self.inner.borrow().ewma_stddev()
    }

    #[jsg_method]
    pub fn ewma_error_rate(&self) -> f64 {
        self.inner.borrow().ewma_error_rate()
    }

    /// # Errors
    /// A `RangeError` if `percentile` is not in (0, 100].
    #[jsg_method]
    pub fn percentile(&self, percentile: f64) -> jsg::Result<f64> {
        self.percentile_big_int(percentile)
            .map(|value| value as f64)
    }

    /// # Errors
    /// A `RangeError` if `percentile` is not in (0, 100].
    #[jsg_method]
    pub fn percentile_big_int(&self, percentile: f64) -> jsg::Result<i64> {
        self.inner
            .borrow()
            .percentile(percentile)
            .map_err(|e| to_jsg_error(&e))
    }

    /// The percentile distribution as a flat array of `[percentile, value, ...]` pairs, in
    /// iteration order. A percentile can repeat; a `Map` built from the pairs keeps the last value.
    #[jsg_method]
    pub fn percentiles(&self) -> Vec<f64> {
        let percentiles = self.inner.borrow().percentiles();
        let mut pairs = Vec::with_capacity(2 * percentiles.len());
        for (percentile, value) in percentiles {
            pairs.extend([percentile, value as f64]);
        }
        pairs
    }

    /// The percentile distribution of `percentiles()`, with the values without loss of precision.
    #[jsg_method]
    pub fn percentiles_big_int(&self) -> PercentilesBigInt {
        let (percentiles, values) = self.inner.borrow().percentiles().into_iter().unzip();
        PercentilesBigInt {
            percentiles,
            values,
        }
    }

    /// Records a number, which the TypeScript layer has checked is a safe integer of at least 0,
    /// so the conversion is exact. Taking an `f64` gives the method a Fast API callback.
    #[jsg_method]
    #[expect(clippy::cast_possible_truncation, reason = "`value` is a safe integer")]
    pub fn record(&self, value: f64) {
        self.inner.borrow_mut().record(value as i64);
    }

    #[jsg_method]
    pub fn record_big_int(&self, value: i64) {
        self.inner.borrow_mut().record(value);
    }

    /// Records the time since the previous call; `now` is the current time in milliseconds since
    /// the epoch, from `Date.now()`.
    #[jsg_method]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "`now` is a positive integral number of milliseconds"
    )]
    pub fn record_delta(&self, now: f64) {
        self.inner
            .borrow_mut()
            .record_delta((now as u64).saturating_mul(1_000_000));
    }

    /// # Errors
    /// Fails if `other` is this histogram and it cannot be copied.
    #[jsg_method]
    pub fn add(&self, lock: &mut jsg::Lock<'_>, other: jsg::Rc<Self>) -> jsg::Result<()> {
        let other = other.get(lock);
        if std::ptr::eq(self, other) {
            // Adding a histogram to itself doubles it, as `hdr_add()` does in Node.js.
            let copy = self
                .inner
                .borrow()
                .snapshot()
                .map_err(|e| to_jsg_error(&e))?;
            self.inner.borrow_mut().add(&copy);
        } else {
            self.inner.borrow_mut().add(&other.inner.borrow());
        }
        Ok(())
    }

    #[jsg_method]
    pub fn reset(&self) {
        self.inner.borrow_mut().reset();
    }
}

/// The `node-internal:histogram` module object.
#[jsg_resource]
pub struct HistogramModule;

#[jsg_resource(nested_types(Histogram))]
impl HistogramModule {
    pub fn new(lock: &mut jsg::Lock<'_>) -> jsg::Rc<Self> {
        jsg::Rc::new(lock, Self {})
    }
}

#[cfg(test)]
#[path = "histogram-test.rs"]
mod tests;
