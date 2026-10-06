/**
 * A hand-rolled SVG line chart for the monitoring page (docs/MONITORING.md
 * §9 q4: no chart library, matching the dashboard's dependency habit).
 *
 * Drawn at the container's measured pixel width rather than scaled through a
 * viewBox, so strokes and labels stay crisp at every size. Static: no
 * transitions, no filters — the mobile GPU budget (globals.css) has no room
 * for animated SVG on a page that refreshes every 5 seconds.
 *
 * The x axis is the *requested* range, not the data's extent: a server that
 * was down for the first half of the window shows as an empty first half,
 * and missing buckets inside the range break the line (`linePath`).
 */
import { useEffect, useId, useRef, useState, type PointerEvent } from "react";

import {
  formatTimeTick,
  linePath,
  nearestIndex,
  niceTicks,
  scale,
  seriesMax,
  timeTicks,
} from "../lib/chart";
import { Panel, cx } from "./ui";

export type ChartLine = {
  label: string;
  values: Array<number | null>;
  /** A CSS color; the `--chart-*` tokens follow the theme. */
  color: string;
  /** What the line measures — shown when its legend entry is hovered,
   *  focused or tapped. */
  description?: string;
};

const HEIGHT = 168;
const M = { top: 10, right: 12, bottom: 22, left: 52 };
/** Half a time label's width, roughly: closer than this to an edge clips. */
const EDGE_PX = 28;

/** Width of the element, kept current with a ResizeObserver. */
function useWidth<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.clientWidth);
    const ro = new ResizeObserver(([entry]) => {
      if (entry) setWidth(Math.round(entry.contentRect.width));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return [ref, width] as const;
}

export function LineChart({
  title,
  ts,
  lines,
  from,
  to,
  bucketMs,
  format,
  note,
}: {
  title: string;
  ts: number[];
  lines: ChartLine[];
  from: number;
  to: number;
  bucketMs: number;
  /** Formats values for the axis, legend and tooltip. */
  format: (v: number) => string;
  /** Shown instead of the plot when every line is empty. */
  note?: string;
}) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);
  // The legend entry whose description is showing. Hover for a mouse, focus
  // for a keyboard, tap to toggle on touch (no hover there).
  const [explained, setExplained] = useState<number | null>(null);
  const titleId = useId();
  const explainId = useId();

  const plotW = Math.max(0, width - M.left - M.right);
  const plotH = HEIGHT - M.top - M.bottom;
  const yTicks = niceTicks(seriesMax(lines.map((l) => l.values)));
  const yMax = yTicks[yTicks.length - 1]!;
  const x = scale(from, to, M.left, M.left + plotW);
  const y = scale(0, yMax, M.top + plotH, M.top);
  const hasData = lines.some((l) => l.values.some((v) => v !== null));

  // The newest bucket that has a value, per line — what the legend shows.
  const latest = lines.map((l) => {
    for (let i = l.values.length - 1; i >= 0; i--) {
      const v = l.values[i];
      if (v !== null && v !== undefined) return v;
    }
    return null;
  });

  const onMove = (e: PointerEvent<SVGSVGElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const t = from + ((px - M.left) / Math.max(1, plotW)) * (to - from);
    const i = nearestIndex(ts, t);
    // Only snap to a bucket that is actually near the pointer; in a gap the
    // crosshair should say "no data", not borrow a value from minutes away.
    setHover(i >= 0 && Math.abs(ts[i]! - t) <= bucketMs ? i : null);
  };

  const hoverX = hover !== null ? x(ts[hover]!) : 0;
  const summary = lines
    .map((l, i) => `${l.label} ${latest[i] === null ? "no data" : format(latest[i]!)}`)
    .join(", ");

  return (
    <Panel className="p-4">
      <div className="mb-2 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h3 id={titleId} className="text-sm font-semibold">
          {title}
        </h3>
        <ul className="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-(--text-muted)">
          {lines.map((l, i) => (
            <li
              key={l.label}
              className={cx(
                "inline-flex items-center gap-1.5",
                l.description && "cursor-help rounded outline-offset-2",
              )}
              {...(l.description
                ? {
                    tabIndex: 0,
                    "aria-describedby": explained === i ? explainId : undefined,
                    onPointerEnter: (e: PointerEvent<HTMLLIElement>) => {
                      if (e.pointerType === "mouse") setExplained(i);
                    },
                    onPointerLeave: (e: PointerEvent<HTMLLIElement>) => {
                      if (e.pointerType === "mouse") setExplained(null);
                    },
                    onClick: () => setExplained((cur) => (cur === i ? null : i)),
                    onFocus: () => setExplained(i),
                    onBlur: () => setExplained(null),
                  }
                : {})}
            >
              <span aria-hidden className="size-2 rounded-full" style={{ background: l.color }} />
              <span
                className={cx(
                  l.description &&
                    "underline decoration-dotted decoration-(--text-faint) underline-offset-2",
                )}
              >
                {l.label}
              </span>
              <span className="font-medium text-(--text) tabular-nums">
                {latest[i] === null ? "—" : format(latest[i]!)}
              </span>
            </li>
          ))}
        </ul>
      </div>

      <div ref={ref} className="relative" style={{ height: HEIGHT }}>
        {width > 0 && (
          <svg
            width={width}
            height={HEIGHT}
            role="img"
            aria-labelledby={titleId}
            aria-description={summary}
            className="block touch-pan-y select-none"
            onPointerMove={onMove}
            onPointerDown={onMove}
            onPointerLeave={() => setHover(null)}
          >
            {yTicks.map((v) => (
              <g key={v}>
                <line
                  x1={M.left}
                  x2={M.left + plotW}
                  y1={y(v)}
                  y2={y(v)}
                  stroke="var(--chart-grid)"
                  strokeWidth={1}
                />
                <text
                  x={M.left - 6}
                  y={y(v)}
                  dy="0.32em"
                  textAnchor="end"
                  fontSize={10}
                  fill="var(--text-faint)"
                >
                  {format(v)}
                </text>
              </g>
            ))}
            {timeTicks(from, to).map((t) => (
              <text
                key={t}
                x={x(t)}
                y={HEIGHT - 6}
                // A tick near either end of the axis anchors inward, or its
                // label is clipped by the edge of the SVG.
                textAnchor={
                  x(t) > M.left + plotW - EDGE_PX
                    ? "end"
                    : x(t) < M.left + EDGE_PX
                      ? "start"
                      : "middle"
                }
                fontSize={10}
                fill="var(--text-faint)"
              >
                {formatTimeTick(t, to - from)}
              </text>
            ))}
            {hasData &&
              lines.map((l) => (
                <path
                  key={l.label}
                  d={linePath(ts, l.values, x, y, bucketMs * 1.5)}
                  fill="none"
                  stroke={l.color}
                  strokeWidth={1.6}
                  strokeLinejoin="round"
                  strokeLinecap="round"
                />
              ))}
            {hover !== null && (
              <line
                x1={hoverX}
                x2={hoverX}
                y1={M.top}
                y2={M.top + plotH}
                stroke="var(--text-faint)"
                strokeDasharray="3 3"
              />
            )}
          </svg>
        )}

        {explained !== null && lines[explained]?.description && (
          // Over the plot rather than beside the legend entry: full panel
          // width never clips at a screen edge, whichever entry it is.
          <div
            id={explainId}
            role="tooltip"
            className="pointer-events-none absolute inset-x-0 top-0 z-20 rounded-lg border
              border-(--surface-border) bg-(--surface-strong) px-3 py-2 text-xs shadow-md backdrop-blur-md"
          >
            <p className="mb-0.5 flex items-center gap-1.5 font-semibold">
              <span
                aria-hidden
                className="size-2 rounded-full"
                style={{ background: lines[explained].color }}
              />
              {lines[explained].label}
            </p>
            <p className="text-(--text-muted)">{lines[explained].description}</p>
          </div>
        )}

        {!hasData && (
          <p className="absolute inset-0 grid place-items-center text-sm text-(--text-faint)">
            {note ?? "No data in this range."}
          </p>
        )}

        {hover !== null && (
          <div
            className="pointer-events-none absolute top-1 z-10 rounded-lg border border-(--surface-border)
              bg-(--surface-strong) px-2.5 py-1.5 text-xs shadow-md backdrop-blur-md"
            style={hoverX > width / 2 ? { right: width - hoverX + 8 } : { left: hoverX + 8 }}
          >
            <p className="mb-0.5 text-(--text-muted)">
              {new Date(ts[hover]!).toLocaleString(undefined, {
                month: "short",
                day: "numeric",
                hour: "2-digit",
                minute: "2-digit",
                second: bucketMs < 60_000 ? "2-digit" : undefined,
              })}
            </p>
            {lines.map((l) => {
              const v = l.values[hover];
              return (
                <p key={l.label} className="flex items-center gap-1.5 whitespace-nowrap">
                  <span
                    aria-hidden
                    className="size-2 rounded-full"
                    style={{ background: l.color }}
                  />
                  {l.label}
                  <span className="ml-auto pl-3 font-medium tabular-nums">
                    {v === null || v === undefined ? "—" : format(v)}
                  </span>
                </p>
              );
            })}
          </div>
        )}
      </div>
    </Panel>
  );
}
