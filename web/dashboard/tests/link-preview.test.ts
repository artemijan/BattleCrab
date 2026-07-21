/**
 * The link-preview card, checked against the **built** output rather than the
 * source — the bundler rewrites index.html, and a card that only works before
 * the build is a card that does not work.
 *
 * None of this is visible in the app. A broken preview shows up when someone
 * posts the link somewhere public, which is the worst time to find out.
 */
import { describe, expect, test } from "bun:test";

import { STATUS } from "../src/lib/status";

const DIST = new URL("../dist", import.meta.url).pathname;

const distBuilt = await Bun.file(`${DIST}/index.html`).exists();

function skip(): boolean {
  if (!distBuilt) {
    console.warn("skipped: run `bun run build` first");
    return true;
  }
  return false;
}

/** Reads the dimensions out of a JPEG's start-of-frame marker. */
function jpegSize(bytes: Uint8Array): { width: number; height: number } | null {
  // Skip the SOI marker, then walk segment headers.
  let offset = 2;
  while (offset < bytes.length - 9) {
    if (bytes[offset] !== 0xff) {
      offset++;
      continue;
    }
    const marker = bytes[offset + 1]!;
    // SOF0..SOF15 carry the frame dimensions; C4/C8/CC are other segments that
    // fall inside the same numeric range.
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      return {
        height: (bytes[offset + 5]! << 8) | bytes[offset + 6]!,
        width: (bytes[offset + 7]! << 8) | bytes[offset + 8]!,
      };
    }
    offset += 2 + ((bytes[offset + 2]! << 8) | bytes[offset + 3]!);
  }
  return null;
}

describe("link preview", () => {
  test("the built page declares an absolute og:image that exists in dist", async () => {
    if (skip()) return;

    const html = await Bun.file(`${DIST}/index.html`).text();
    const match = html.match(/<meta\s+property="og:image"\s+content="([^"]+)"/);

    expect(match).not.toBeNull();
    const url = match![1]!;

    // Crawlers do not resolve relative paths the way a browser does; a relative
    // og:image is the single most common reason a card renders blank.
    expect(url.startsWith("https://")).toBe(true);

    // Whatever the URL's last segment is, it has to be a file the server can
    // actually serve — build.ts copies it, unhashed, for exactly this reason.
    const filename = url.split("/").pop()!;
    expect(await Bun.file(`${DIST}/${filename}`).exists()).toBe(true);
  });

  test("the declared dimensions match the real image", async () => {
    if (skip()) return;

    const html = await Bun.file(`${DIST}/index.html`).text();
    const declared = {
      width: Number(html.match(/property="og:image:width"\s+content="(\d+)"/)?.[1]),
      height: Number(html.match(/property="og:image:height"\s+content="(\d+)"/)?.[1]),
    };

    const bytes = new Uint8Array(await Bun.file(`${DIST}/og-image.jpg`).arrayBuffer());
    const actual = jpegSize(bytes);

    // Platforms lay the card out from the declared size before the image
    // arrives, so a mismatch shows as a jump or a letterboxed card.
    expect(actual).toEqual(declared);
    // And the ratio Twitter's summary_large_image expects.
    expect(declared.width).toBe(1200);
    expect(declared.height).toBe(630);
  });

  test("the card is small enough to unfurl quickly", async () => {
    if (skip()) return;

    const size = Bun.file(`${DIST}/og-image.jpg`).size;
    expect(size).toBeGreaterThan(10_000); // not a truncated write
    // Well inside every platform's limit (Twitter's is 5 MB), and small enough
    // that a chat unfurl is instant. PNG at this size was ~470 KiB.
    expect(size).toBeLessThan(300_000);
  });

  /**
   * index.html cannot import `STATUS`, so the phase is written out twice. This
   * is what stops the two drifting: a shared link still advertising "early
   * alpha" months into open beta is worse than one carrying no status at all,
   * and nobody re-reads their own meta tags.
   */
  test("the preview description names the same phase as the site", async () => {
    if (skip()) return;

    const html = await Bun.file(`${DIST}/index.html`).text();
    const description = html.match(/property="og:description"\s+content="([^"]+)"/)?.[1] ?? "";

    expect(description.toLowerCase()).toContain(STATUS.phase.toLowerCase());
  });

  test("the wide-card type is declared", async () => {
    if (skip()) return;

    const html = await Bun.file(`${DIST}/index.html`).text();
    // Twitter reads og:* for content but needs this for the layout; without it
    // the card is a small square thumbnail.
    expect(html).toContain('name="twitter:card" content="summary_large_image"');
    expect(html).toMatch(/property="og:url"\s+content="https:\/\//);
    expect(html).toMatch(/property="og:image:alt"/);
  });
});
