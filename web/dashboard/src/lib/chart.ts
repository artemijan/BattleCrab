/**
 * The arithmetic behind the hand-rolled SVG charts (`components/LineChart`).
 *
 * Kept free of React and the DOM so it is unit-testable, and because this is
 * where chart bugs actually live: an axis that never reaches its data, a line
 * drawn straight across an outage, a rate computed over the wrong interval.
 * See docs/MONITORING.md §5 for the series shapes this consumes.
 */

/** Linear map from `[d0, d1]` to `[r0, r1]`. A zero-width domain maps to the
 *  range's midpoint rather than dividing by zero. */
export function scale(d0: number, d1: number, r0: number, r1: number): (v: number) => number {
  const span = d1 - d0;
  if (span === 0) return () => (r0 + r1) / 2;
  return (v) => r0 + ((v - d0) / span) * (r1 - r0);
}

/**
 * "Nice" tick values covering `[0, max]`, for a value axis: steps of 1, 2 or 5
 * times a power of ten, the last tick at or above `max`, so the line never
 * leaves the plot. Value axes always start at 0 — every series charted here is
 * a rate, a count or a size, and a baseline that floats with the data turns a
 * 2% wobble into what looks like a cliff.
 */
export function niceTicks(max: number, target = 4): number[] {
  if (!(max > 0) || !Number.isFinite(max)) return [0, 1];
  const rough = max / target;
  const pow = 10 ** Math.floor(Math.log10(rough));
  const step = [1, 2, 5, 10].map((m) => m * pow).find((s) => s >= rough) ?? 10 * pow;
  const ticks: number[] = [];
  for (let v = 0; v < max + step * 1e-9; v += step) ticks.push(Number(v.toPrecision(12)));
  if (ticks[ticks.length - 1]! < max) ticks.push(Number((ticks.length * step).toPrecision(12)));
  return ticks;
}

/**
 * Time ticks: epoch-ms values at a round step (minutes, hours, days) that puts
 * roughly `target` ticks across `[from, to]`. Aligned to local midnight for
 * day steps so a week reads as days.
 */
export function timeTicks(from: number, to: number, target = 5): number[] {
  const MIN = 60_000;
  const HOUR = 60 * MIN;
  const steps = [MIN, 5 * MIN, 15 * MIN, 30 * MIN, HOUR, 3 * HOUR, 6 * HOUR, 12 * HOUR, 24 * HOUR];
  const span = to - from;
  if (!(span > 0)) return [];
  const step =
    steps.find((s) => span / s <= target) ?? 24 * HOUR * Math.ceil(span / (24 * HOUR) / target);
  // Align to the local clock, so ticks land on :00/:15 and on midnight.
  const offset = new Date(from).getTimezoneOffset() * MIN;
  let t = Math.ceil((from - offset) / step) * step + offset;
  const out: number[] = [];
  for (; t <= to; t += step) out.push(t);
  return out;
}

/**
 * SVG path data for one series, broken into separate runs wherever the data
 * is missing: a `null` value, or a jump in `ts` larger than `maxGapMs`. A
 * stopped server must show as a hole, not as a straight line from its last
 * sample to its first after restart.
 */
export function linePath(
  ts: number[],
  values: Array<number | null>,
  x: (t: number) => number,
  y: (v: number) => number,
  maxGapMs: number,
): string {
  let d = "";
  let prevTs: number | null = null;
  for (let i = 0; i < ts.length; i++) {
    const t = ts[i]!;
    const v = values[i];
    if (v === null || v === undefined || !Number.isFinite(v)) {
      prevTs = null;
      continue;
    }
    const join = prevTs !== null && t - prevTs <= maxGapMs;
    // A new run opens with a zero-length segment: with round line caps it
    // renders as a dot, so a lone sample between two gaps is still visible.
    d += join
      ? `L${x(t).toFixed(1)},${y(v).toFixed(1)}`
      : `M${x(t).toFixed(1)},${y(v).toFixed(1)}l0,0`;
    prevTs = t;
  }
  return d;
}

/**
 * Per-second rate of a summed delta series: each bucket's sum over the real
 * time it covered. `intervalMs` is the bucket's summed sample intervals, which
 * is what keeps a bucket with a missing sample from reading as a dip.
 */
export function perSecond(
  values: Array<number | null>,
  intervalMs: number[],
): Array<number | null> {
  return values.map((v, i) => {
    const ms = intervalMs[i] ?? 0;
    return v === null || !(ms > 0) ? null : (v * 1000) / ms;
  });
}

/** CPU use as a percentage of one core: CPU-µs over wall-µs. */
export function cpuPercent(
  cpuMicros: Array<number | null>,
  intervalMs: number[],
): Array<number | null> {
  return cpuMicros.map((v, i) => {
    const ms = intervalMs[i] ?? 0;
    return v === null || !(ms > 0) ? null : v / (ms * 10);
  });
}

/** Element-wise `a / b`, null where either side is missing or `b` is 0. */
export function ratio(
  a: Array<number | null>,
  b: Array<number | null>,
  factor = 1,
): Array<number | null> {
  return a.map((v, i) => {
    const d = b[i];
    return v === null || d === null || d === undefined || d === 0 ? null : (v / d) * factor;
  });
}

/** The largest finite value across series, for the y-axis. */
export function seriesMax(series: Array<Array<number | null>>): number {
  let max = 0;
  for (const s of series)
    for (const v of s) if (v !== null && Number.isFinite(v) && v > max) max = v;
  return max;
}

/** Index of the bucket nearest `t` (by timestamp), or -1 for no data. */
export function nearestIndex(ts: number[], t: number): number {
  if (ts.length === 0) return -1;
  let lo = 0;
  let hi = ts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (ts[mid]! < t) lo = mid + 1;
    else hi = mid;
  }
  if (lo > 0 && t - ts[lo - 1]! < ts[lo]! - t) return lo - 1;
  return lo;
}

/* ------------------------------- Formatting ------------------------------- */

const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(v: number): string {
  let i = 0;
  let n = v;
  while (Math.abs(n) >= 1024 && i < UNITS.length - 1) {
    n /= 1024;
    i++;
  }
  return `${trim(n)} ${UNITS[i]}`;
}

/** 1234 → "1.23k", 0.5 → "0.5". Short enough for an axis label. */
export function formatCount(v: number): string {
  const abs = Math.abs(v);
  if (abs >= 1e9) return `${trim(v / 1e9)}G`;
  if (abs >= 1e6) return `${trim(v / 1e6)}M`;
  if (abs >= 1e3) return `${trim(v / 1e3)}k`;
  return trim(v);
}

function trim(n: number): string {
  const abs = Math.abs(n);
  const digits = abs >= 100 || abs === 0 ? 0 : abs >= 10 ? 1 : 2;
  return Number(n.toFixed(digits)).toString();
}

export function formatDuration(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** Axis label for a time tick: clock time within a day, date beyond. */
export function formatTimeTick(t: number, spanMs: number): string {
  const date = new Date(t);
  if (spanMs > 36 * 3_600_000) {
    return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
  return date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}
