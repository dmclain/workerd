// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

// Scenarios for perf_hooks histograms, shared by perf-hooks-histogram-nodejs-test.js and
// perf-hooks-histogram-golden.mjs, which runs them in Node.js to generate the expected results.
// They must stay deterministic: no clocks, and datasets from integer arithmetic only.

import { createHistogram } from 'node:perf_hooks';
import { inspect } from 'node:util';

// MINSTD: exact in doubles, since the product stays below 2^53.
function minstd(seed) {
  let state = seed;
  return () => {
    state = (state * 48271) % 2147483647;
    return state;
  };
}

function generate(seed, n, f) {
  const next = minstd(seed);
  return Array.from({ length: n }, () => f(next()));
}

const datasets = {
  one: [7],
  zero: [0],
  edges: [1, 2, 3, 2047, 2048, 2049, 1e6, 2 ** 40, Number.MAX_SAFE_INTEGER],
  constant: Array(100).fill(42),
  uniform: generate(1, 2000, (s) => 1 + (s % 10000)),
  // Values spread over many orders of magnitude.
  wide: generate(7, 1500, (s) => 1 + (s % 2 ** (1 + (s % 50)))),
  // Mostly small values with a long tail.
  tail: generate(11, 1000, (s) =>
    s % 100 === 0 ? 1 + (s % 1e9) : 1 + (s % 50)
  ),
};

const layouts = {
  default: undefined,
  small: { lowest: 1, highest: 11, figures: 1 },
  custom: { lowest: 10, highest: 100_000, figures: 2 },
  precise: { lowest: 1, highest: 1e6, figures: 5 },
  bigint: { lowest: 1000n, highest: 2n ** 62n, figures: 3 },
  ewma: { halfLife: 10, threshold: 5000 },
};

const kPercentiles = [0.0001, 1, 25, 50, 75, 90, 99, 99.9, 99.99, 100];

function dump(h) {
  return {
    count: h.count,
    countBigInt: String(h.countBigInt),
    min: h.min,
    minBigInt: String(h.minBigInt),
    max: h.max,
    maxBigInt: String(h.maxBigInt),
    mean: h.mean,
    stddev: h.stddev,
    skewness: h.skewness,
    kurtosis: h.kurtosis,
    exceeds: h.exceeds,
    exceedsBigInt: String(h.exceedsBigInt),
    ewmaMean: h.ewmaMean,
    ewmaStddev: h.ewmaStddev,
    ewmaErrorRate: h.ewmaErrorRate,
    resetCount: h.resetCount,
    percentiles: [...h.percentiles],
    percentilesBigInt: [...h.percentilesBigInt].map(([k, v]) => [k, String(v)]),
    percentile: h.count === 0 ? [] : kPercentiles.map((p) => h.percentile(p)),
    percentileBigInt:
      h.count === 0
        ? []
        : kPercentiles.map((p) => String(h.percentileBigInt(p))),
    toJSON: h.toJSON(),
    inspect: inspect(h),
  };
}

function error(f) {
  try {
    f();
  } catch (e) {
    return { name: e.name, code: e.code, message: e.message };
  }
  return null;
}

export function run() {
  const results = {};

  results.empty = dump(createHistogram());

  for (const [layoutName, options] of Object.entries(layouts)) {
    for (const [dataName, values] of Object.entries(datasets)) {
      const h = createHistogram(options);
      for (const v of values) h.record(v);
      results[`${layoutName}/${dataName}`] = dump(h);
    }
  }

  // Values out of the layout's range count as exceeds.
  {
    const h = createHistogram({ lowest: 10, highest: 1000, figures: 2 });
    for (const v of [1, 5, 10, 999, 1000, 1001, 5000, 1e9]) h.record(v);
    results.outOfRange = dump(h);
  }

  // BigInt records, including values beyond Number.MAX_SAFE_INTEGER.
  {
    const h = createHistogram({ highest: 2n ** 62n });
    for (const v of [
      1n,
      3n,
      2n ** 53n + 1n,
      2n ** 61n,
      2n ** 62n,
      2n ** 63n - 1n,
    ]) {
      h.record(v);
    }
    results.bigintRecords = dump(h);
  }

  // add(): another histogram, a histogram of another layout, and itself.
  {
    const a = createHistogram();
    const b = createHistogram();
    for (const v of datasets.uniform) a.record(v);
    for (const v of datasets.tail) b.record(v);
    b.record(2n ** 60n);
    a.add(b);
    results.addOther = dump(a);
    a.add(a);
    results.addSelf = dump(a);

    const c = createHistogram({ lowest: 10, highest: 100_000, figures: 2 });
    for (const v of datasets.wide) c.record(v);
    const d = createHistogram();
    d.add(c);
    results.addOtherLayout = dump(d);
  }

  // reset() clears values and exceeds, and counts resets.
  {
    const h = createHistogram({
      lowest: 1,
      highest: 100,
      figures: 2,
      halfLife: 3,
    });
    for (const v of [1, 50, 1000]) h.record(v);
    h.reset();
    results.afterReset = dump(h);
    for (const v of [2, 3]) h.record(v);
    h.reset();
    h.record(4);
    results.afterTwoResets = dump(h);
  }

  // A threshold that is not a safe integer.
  results.thresholdEdges = [Infinity, 1e300, NaN, 2.5].map((threshold) => {
    const h = createHistogram({ halfLife: 10, threshold });
    for (const v of datasets.tail) h.record(v);
    return dump(h);
  });

  // Argument validation.
  const h = createHistogram();
  const other = createHistogram();
  results.errors = {
    optionsString: error(() => createHistogram('hello')),
    optionsNull: error(() => createHistogram(null)),
    lowestZero: error(() => createHistogram({ lowest: 0 })),
    lowestFraction: error(() => createHistogram({ lowest: 1.5 })),
    lowestBigZero: error(() => createHistogram({ lowest: 0n })),
    lowestString: error(() => createHistogram({ lowest: '1' })),
    highestTooSmall: error(() => createHistogram({ lowest: 10, highest: 19 })),
    highestBigTooLarge: error(() => createHistogram({ highest: 2n ** 63n })),
    highestUnsafe: error(() => createHistogram({ highest: 2 ** 53 })),
    figuresZero: error(() => createHistogram({ figures: 0 })),
    figuresSix: error(() => createHistogram({ figures: 6 })),
    figuresBigInt: error(() => createHistogram({ figures: 3n })),
    halfLifeNegative: error(() => createHistogram({ halfLife: -1 })),
    halfLifeString: error(() => createHistogram({ halfLife: '1' })),
    thresholdNegative: error(() => createHistogram({ threshold: -1 })),
    recordNegative: error(() => h.record(-1)),
    recordFraction: error(() => h.record(1.5)),
    recordUnsafe: error(() => h.record(2 ** 53)),
    recordString: error(() => h.record('1')),
    recordUndefined: error(() => h.record()),
    recordBigNegative: error(() => h.record(-1n)),
    recordBigTooLarge: error(() => h.record(2n ** 63n)),
    percentileZero: error(() => h.percentile(0)),
    percentileAbove: error(() => h.percentile(100.5)),
    percentileNaN: error(() => h.percentile(NaN)),
    percentileString: error(() => h.percentile('50')),
    percentileBigIntZero: error(() => h.percentileBigInt(0)),
    addNumber: error(() => h.add(1)),
    addObject: error(() => h.add({})),
    illegalConstructor: error(() => new h.constructor()),
    illegalSuperConstructor: error(
      () => new (Object.getPrototypeOf(h.constructor))()
    ),
    invalidThisGetter: error(() =>
      Object.getOwnPropertyDescriptor(
        Object.getPrototypeOf(Object.getPrototypeOf(h)),
        'count'
      ).get.call({})
    ),
    invalidThisRecord: error(() => h.record.call({}, 1)),
    invalidThisAdd: error(() => h.add.call({}, other)),
  };

  results.shape = {
    constructorName: h.constructor.name,
    superName: Object.getPrototypeOf(h.constructor).name,
    inspectShallow: inspect(h, { depth: -1 }),
    jsonString: JSON.stringify(createHistogram()),
  };

  return results;
}
