/**
 * The chart arithmetic (`src/lib/chart.ts`). No browser needed: these are the
 * places a chart lies without looking broken — an axis that stops short of
 * its data, a line drawn across an outage, a rate over the wrong interval.
 */
import { describe, expect, test } from "bun:test";

import {
  cpuPercent,
  formatBytes,
  formatCount,
  formatDuration,
  linePath,
  nearestIndex,
  niceTicks,
  perSecond,
  ratio,
  scale,
  seriesMax,
  timeTicks,
} from "../src/lib/chart";

describe("niceTicks", () => {
  test("covers the max with round steps from zero", () => {
    for (const max of [0.3, 1, 7, 42, 99.9, 100, 101, 1234, 5e9]) {
      const ticks = niceTicks(max);
      expect(ticks[0]).toBe(0);
      expect(ticks[ticks.length - 1]!).toBeGreaterThanOrEqual(max);
      const step = ticks[1]! - ticks[0]!;
      const mantissa = step / 10 ** Math.floor(Math.log10(step));
      expect([1, 2, 5]).toContain(Math.round(mantissa));
      expect(ticks.length).toBeLessThanOrEqual(7);
    }
  });

  test("an empty or flat series still gets an axis", () => {
    expect(niceTicks(0)).toEqual([0, 1]);
    expect(niceTicks(Number.NaN)).toEqual([0, 1]);
  });
});

describe("linePath", () => {
  const x = (t: number) => t;
  const y = (v: number) => v;

  test("breaks at nulls and at gaps in time, never bridging an outage", () => {
    const ts = [0, 10, 20, 30, 100, 110];
    const values = [1, 2, null, 4, 5, 6];
    expect(linePath(ts, values, x, y, 15)).toBe(
      "M0.0,1.0l0,0L10.0,2.0M30.0,4.0l0,0M100.0,5.0l0,0L110.0,6.0",
    );
  });

  test("a series with no data draws nothing", () => {
    expect(linePath([0, 1], [null, null], x, y, 10)).toBe("");
  });
});

describe("rates", () => {
  test("per-second divides by the time the bucket actually covered", () => {
    // Second bucket lost a sample: 5s of data, not 10s. Its rate is the same
    // as the first's, not half of it.
    expect(perSecond([100, 50, null], [10_000, 5_000, 10_000])).toEqual([10, 10, null]);
    expect(perSecond([5], [0])).toEqual([null]);
  });

  test("cpu percent is cpu-µs over wall-µs", () => {
    // 2.5 s of CPU in a 5 s interval is half a core.
    expect(cpuPercent([2_500_000], [5_000])).toEqual([50]);
  });

  test("ratio guards the zero denominator", () => {
    expect(ratio([10, 5, null], [2, 0, 3], 0.5)).toEqual([2.5, null, null]);
  });
});

test("scale maps linearly and survives a zero-width domain", () => {
  expect(scale(0, 10, 100, 200)(5)).toBe(150);
  expect(scale(3, 3, 0, 10)(3)).toBe(5);
});

test("seriesMax ignores nulls and non-finite values", () => {
  expect(
    seriesMax([
      [1, null, 3],
      [Number.POSITIVE_INFINITY, 2],
    ]),
  ).toBe(3);
  expect(seriesMax([])).toBe(0);
});

test("nearestIndex picks the closest bucket", () => {
  const ts = [0, 10, 20];
  expect(nearestIndex(ts, -5)).toBe(0);
  expect(nearestIndex(ts, 4)).toBe(0);
  expect(nearestIndex(ts, 6)).toBe(1);
  expect(nearestIndex(ts, 99)).toBe(2);
  expect(nearestIndex([], 1)).toBe(-1);
});

test("time ticks fall inside the range at a round step", () => {
  const from = Date.UTC(2026, 7, 14, 10, 7);
  const to = from + 3_600_000;
  const ticks = timeTicks(from, to);
  expect(ticks.length).toBeGreaterThan(1);
  expect(ticks.length).toBeLessThanOrEqual(5);
  for (const t of ticks) {
    expect(t).toBeGreaterThanOrEqual(from);
    expect(t).toBeLessThanOrEqual(to);
  }
  expect(ticks[1]! - ticks[0]!).toBe(15 * 60_000);
  expect(timeTicks(5, 5)).toEqual([]);
});

test("formatting", () => {
  expect(formatBytes(512)).toBe("512 B");
  expect(formatBytes(1536)).toBe("1.5 KB");
  expect(formatBytes(64 * 1024 * 1024)).toBe("64 MB");
  expect(formatCount(1234)).toBe("1.23k");
  expect(formatCount(0.5)).toBe("0.5");
  expect(formatCount(2_500_000)).toBe("2.5M");
  expect(formatDuration(90)).toBe("1m");
  expect(formatDuration(3 * 3600 + 120)).toBe("3h 2m");
  expect(formatDuration(2 * 86400 + 3600)).toBe("2d 1h");
});
