// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// The `Histogram` and `RecordableHistogram` classes of `node:perf_hooks`, ported from Node.js's
// lib/internal/histogram.js over the native histogram of `node-internal:histogram`, which matches
// Node.js's results exactly.

import { default as histogramImpl } from 'node-internal:histogram';
import {
  ERR_ILLEGAL_CONSTRUCTOR,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_THIS,
  ERR_OUT_OF_RANGE,
  NodeRangeError,
} from 'node-internal:internal_errors';
import {
  validateInteger,
  validateNumber,
  validateObject,
} from 'node-internal:validators';
import {
  customInspectSymbol,
  inspect,
  type InspectOptionsStylized,
} from 'node-internal:internal_inspect';

type Handle = InstanceType<typeof histogramImpl.Histogram>;

const kSkipThrow = Symbol('kSkipThrow');
const kMaxInt64 = 9_223_372_036_854_775_807n;

let handleOf: (value: unknown) => Handle | undefined;
let isRecordable: (value: unknown) => boolean;

function histogramHandle(value: unknown): Handle {
  const handle = handleOf(value);
  if (handle === undefined) throw new ERR_INVALID_THIS('Histogram');
  return handle;
}

function recordableHandle(value: unknown): Handle {
  if (!isRecordable(value)) throw new ERR_INVALID_THIS('RecordableHistogram');
  return histogramHandle(value);
}

function validatePercentile(percentile: unknown): asserts percentile is number {
  validateNumber(percentile, 'percentile');
  if (Number.isNaN(percentile) || percentile <= 0 || percentile > 100) {
    throw new ERR_OUT_OF_RANGE('percentile', '> 0 && <= 100', percentile);
  }
}

export class Histogram {
  #handle: Handle;

  static {
    handleOf = (value: unknown): Handle | undefined =>
      typeof value === 'object' && value !== null && #handle in value
        ? value.#handle
        : undefined;
  }

  constructor(skipThrowSymbol: symbol, handle: Handle) {
    if (skipThrowSymbol !== kSkipThrow) {
      throw new ERR_ILLEGAL_CONSTRUCTOR();
    }
    this.#handle = handle;
  }

  [customInspectSymbol](
    depth: number,
    options: InspectOptionsStylized
  ): string {
    if (depth < 0) {
      return options.stylize(`[${this.constructor.name}]`, 'special');
    }

    const opts = {
      ...options,
      depth: options.depth == null ? null : options.depth - 1,
    };

    return `Histogram ${inspect(
      {
        min: this.min,
        max: this.max,
        mean: this.mean,
        exceeds: this.exceeds,
        stddev: this.stddev,
        skewness: this.skewness,
        kurtosis: this.kurtosis,
        count: this.count,
        percentiles: this.percentiles,
      },
      opts
    )}`;
  }

  get count(): number {
    return histogramHandle(this).count();
  }

  get countBigInt(): bigint {
    return histogramHandle(this).countBigInt();
  }

  get min(): number {
    return histogramHandle(this).min();
  }

  get minBigInt(): bigint {
    return histogramHandle(this).minBigInt();
  }

  get max(): number {
    return histogramHandle(this).max();
  }

  get maxBigInt(): bigint {
    return histogramHandle(this).maxBigInt();
  }

  get mean(): number {
    return histogramHandle(this).mean();
  }

  get exceeds(): number {
    return histogramHandle(this).exceeds();
  }

  get exceedsBigInt(): bigint {
    return histogramHandle(this).exceedsBigInt();
  }

  // The exponentially weighted moving average of recorded values; 0 unless the histogram was
  // created with a `halfLife`.
  get ewmaMean(): number {
    return histogramHandle(this).ewmaMean();
  }

  get ewmaStddev(): number {
    return histogramHandle(this).ewmaStddev();
  }

  // The EWMA of the fraction of values above `threshold`; 0 unless the histogram was created with
  // both `halfLife` and `threshold`.
  get ewmaErrorRate(): number {
    return histogramHandle(this).ewmaErrorRate();
  }

  get kurtosis(): number {
    return histogramHandle(this).kurtosis();
  }

  get skewness(): number {
    return histogramHandle(this).skewness();
  }

  get stddev(): number {
    return histogramHandle(this).stddev();
  }

  percentile(percentile: number): number {
    const handle = histogramHandle(this);
    validatePercentile(percentile);
    return handle.percentile(percentile);
  }

  percentileBigInt(percentile: number): bigint {
    const handle = histogramHandle(this);
    validatePercentile(percentile);
    return handle.percentileBigInt(percentile);
  }

  get percentiles(): Map<number, number> {
    const pairs = histogramHandle(this).percentiles();
    const map = new Map<number, number>();
    for (let i = 0; i < pairs.length; i += 2) {
      map.set(pairs[i] as number, pairs[i + 1] as number);
    }
    return map;
  }

  get percentilesBigInt(): Map<number, bigint> {
    const { percentiles, values } = histogramHandle(this).percentilesBigInt();
    const map = new Map<number, bigint>();
    for (let i = 0; i < values.length; i++) {
      map.set(percentiles[i] as number, values[i] as bigint);
    }
    return map;
  }

  reset(): void {
    histogramHandle(this).reset();
  }

  // The number of times values were removed from the histogram by `reset()`.
  get resetCount(): number {
    return histogramHandle(this).resetCount();
  }

  toJSON(): Record<string, unknown> {
    return {
      count: this.count,
      min: this.min,
      max: this.max,
      mean: this.mean,
      exceeds: this.exceeds,
      stddev: this.stddev,
      skewness: this.skewness,
      kurtosis: this.kurtosis,
      ewmaMean: this.ewmaMean,
      ewmaStddev: this.ewmaStddev,
      ewmaErrorRate: this.ewmaErrorRate,
      percentiles: Object.fromEntries(this.percentiles),
    };
  }
}

export class RecordableHistogram extends Histogram {
  #recordable = true;

  static {
    isRecordable = (value: unknown): boolean =>
      typeof value === 'object' && value !== null && #recordable in value;
  }

  record(val: number | bigint): void {
    const handle = recordableHandle(this);
    if (typeof val === 'bigint') {
      if (val < 0n || val > kMaxInt64) {
        throw new NodeRangeError('ERR_OUT_OF_RANGE', 'value is out of range');
      }
      handle.recordBigInt(val);
    } else {
      validateInteger(val, 'val', 0);
      handle.record(val);
    }
  }

  // Records the time in nanoseconds since the previous call. The first call records nothing. In
  // Workers, the clock only advances on I/O, so the deltas within one synchronous run are 0.
  recordDelta(): void {
    recordableHandle(this).recordDelta(Date.now());
  }

  add(other: RecordableHistogram): void {
    const handle = recordableHandle(this);
    if (!isRecordable(other)) {
      throw new ERR_INVALID_ARG_TYPE('other', 'RecordableHistogram', other);
    }
    handle.add(histogramHandle(other));
  }
}

export interface CreateHistogramOptions {
  lowest?: number | bigint;
  highest?: number | bigint;
  figures?: number;
  halfLife?: number;
  threshold?: number;
}

function validateBound(
  value: unknown,
  name: string
): asserts value is number | bigint {
  if (typeof value !== 'bigint') {
    validateInteger(value, name, 1, Number.MAX_SAFE_INTEGER);
  } else if (value < 1n || value > kMaxInt64) {
    throw new ERR_OUT_OF_RANGE(name, `>= 1n && <= ${kMaxInt64}n`, value);
  }
}

function validateHistogramOptions(
  lowest: unknown,
  highest: unknown,
  figures: unknown
): asserts lowest is number | bigint {
  validateBound(lowest, 'options.lowest');
  validateBound(highest, 'options.highest');
  const minimumHighest = 2n * BigInt(lowest);
  if (BigInt(highest) < minimumHighest) {
    throw new ERR_OUT_OF_RANGE(
      'options.highest',
      `>= 2 * options.lowest (${minimumHighest}n)`,
      highest
    );
  }
  validateInteger(figures, 'options.figures', 1, 5);
}

export function createHistogram(
  options: CreateHistogramOptions = {}
): RecordableHistogram {
  validateObject(options, 'options');
  const {
    lowest = 1,
    highest = Number.MAX_SAFE_INTEGER,
    figures = 3,
    halfLife = 0,
    threshold = 0,
  } = options;
  validateHistogramOptions(lowest, highest, figures);
  validateNumber(halfLife, 'options.halfLife');
  if (halfLife < 0) {
    throw new ERR_OUT_OF_RANGE('options.halfLife', '>= 0', halfLife);
  }
  validateNumber(threshold, 'options.threshold');
  if (threshold < 0) {
    throw new ERR_OUT_OF_RANGE('options.threshold', '>= 0', threshold);
  }
  return new RecordableHistogram(
    kSkipThrow,
    new histogramImpl.Histogram(lowest, highest, figures, halfLife, threshold)
  );
}
