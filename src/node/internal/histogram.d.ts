// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

// Type definitions for the native (Rust) implementation of `node-internal:histogram`
// (src/rust/api/histogram.rs). Arguments are validated by perf_hooks_histogram.ts.

export class Histogram {
  constructor(
    lowest: number | bigint,
    highest: number | bigint,
    figures: number,
    halfLife: number,
    threshold: number
  );
  count(): number;
  countBigInt(): bigint;
  exceeds(): number;
  exceedsBigInt(): bigint;
  resetCount(): number;
  min(): number;
  minBigInt(): bigint;
  max(): number;
  maxBigInt(): bigint;
  mean(): number;
  stddev(): number;
  skewness(): number;
  kurtosis(): number;
  ewmaMean(): number;
  ewmaStddev(): number;
  ewmaErrorRate(): number;
  percentile(percentile: number): number;
  percentileBigInt(percentile: number): bigint;
  // Flat [percentile, value, ...] pairs.
  percentiles(): number[];
  // The percentiles and their values, in iteration order.
  percentilesBigInt(): { percentiles: number[]; values: bigint[] };
  record(value: number): void;
  recordBigInt(value: bigint): void;
  // `now` is the current time in milliseconds.
  recordDelta(now: number): void;
  add(other: Histogram): void;
  reset(): void;
}

declare const moduleDefault: {
  Histogram: typeof Histogram;
};

export default moduleDefault;
