// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { createHistogram } from 'node:perf_hooks';
import { inspect } from 'node:util';
import { deepStrictEqual, ok, strictEqual, throws } from 'node:assert';
import { run } from 'perf-hooks-histogram-scenarios.js';
import golden from 'perf-hooks-histogram-golden.json';

// Every scenario gives the same results as Node.js (see perf-hooks-histogram-golden.mjs).
export const matchesNodeJs = {
  test() {
    // A JSON round trip, as the golden results took, turns NaN into null.
    const results = JSON.parse(JSON.stringify(run()));
    deepStrictEqual(Object.keys(results), Object.keys(golden.results));
    for (const [name, expected] of Object.entries(golden.results)) {
      deepStrictEqual(results[name], expected, name);
    }
  },
};

// Ported from Node.js's test/parallel/test-perf-hooks-histogram.js, without the parts that need
// monitorEventLoopDelay() or transferring a histogram over a MessageChannel.
export const nodeHistogramTest = {
  test() {
    {
      const h = createHistogram();

      strictEqual(h.min, 9223372036854776000);
      strictEqual(h.minBigInt, 9223372036854775807n);
      strictEqual(h.max, 0);
      strictEqual(h.maxBigInt, 0n);
      strictEqual(h.exceeds, 0);
      strictEqual(h.exceedsBigInt, 0n);
      ok(Number.isNaN(h.mean));
      ok(Number.isNaN(h.stddev));

      strictEqual(h.count, 0);
      strictEqual(h.countBigInt, 0n);

      h.record(1);

      strictEqual(h.count, 1);
      strictEqual(h.countBigInt, 1n);

      [false, '', {}, undefined, null].forEach((i) => {
        throws(() => h.record(i), { code: 'ERR_INVALID_ARG_TYPE' });
      });
      [-1, Number.MAX_SAFE_INTEGER + 1].forEach((i) => {
        throws(() => h.record(i), { code: 'ERR_OUT_OF_RANGE' });
      });

      strictEqual(h.min, 1);
      strictEqual(h.minBigInt, 1n);
      strictEqual(h.max, 1);
      strictEqual(h.maxBigInt, 1n);
      strictEqual(h.exceeds, 0);
      strictEqual(h.mean, 1);
      strictEqual(h.stddev, 0);

      strictEqual(h.percentile(1), 1);
      strictEqual(h.percentile(100), 1);

      strictEqual(h.percentileBigInt(1), 1n);
      strictEqual(h.percentileBigInt(100), 1n);

      deepStrictEqual(
        h.percentiles,
        new Map([
          [0, 1],
          [100, 1],
        ])
      );

      deepStrictEqual(
        h.percentilesBigInt,
        new Map([
          [0, 1n],
          [100, 1n],
        ])
      );

      h.record(2n);
      strictEqual(h.max, 2);
    }

    {
      const h = createHistogram();
      ok(inspect(h, { depth: null }).startsWith('Histogram'));
      strictEqual(inspect(h, { depth: -1 }), '[RecordableHistogram]');
    }

    {
      // Tests that RecordableHistogram is impossible to construct manually
      const h = createHistogram();
      throws(() => new h.constructor(), { code: 'ERR_ILLEGAL_CONSTRUCTOR' });
    }

    {
      ['hello', 1, null].forEach((i) => {
        throws(() => createHistogram(i), { code: 'ERR_INVALID_ARG_TYPE' });
      });

      ['hello', false, null, {}].forEach((i) => {
        throws(() => createHistogram({ lowest: i }), {
          code: 'ERR_INVALID_ARG_TYPE',
        });
        throws(() => createHistogram({ highest: i }), {
          code: 'ERR_INVALID_ARG_TYPE',
        });
        throws(() => createHistogram({ figures: i }), {
          code: 'ERR_INVALID_ARG_TYPE',
        });
      });

      // Number greater than 5 is not allowed
      for (const i of [6, 10]) {
        throws(() => createHistogram({ figures: i }), {
          code: 'ERR_OUT_OF_RANGE',
        });
      }

      createHistogram({ lowest: 1, highest: 11, figures: 1 });
    }

    {
      const h1 = createHistogram();
      const h2 = createHistogram();

      h1.record(1);

      strictEqual(h2.count, 0);
      strictEqual(h1.count, 1);

      h2.add(h1);

      strictEqual(h2.count, 1);

      ['hello', 1, false, {}].forEach((i) => {
        throws(() => h1.add(i), { code: 'ERR_INVALID_ARG_TYPE' });
      });
    }
  },
};

export const recordDelta = {
  async test() {
    const h = createHistogram();
    // The first call only starts the clock.
    h.recordDelta();
    strictEqual(h.count, 0);
    // The clock advances across I/O.
    await scheduler.wait(10);
    h.recordDelta();
    strictEqual(h.count, 1);
    ok(h.min >= 1_000_000, `${h.min}`);
    h.reset();
    // reset() restarts the clock.
    h.recordDelta();
    strictEqual(h.count, 0);
  },
};
