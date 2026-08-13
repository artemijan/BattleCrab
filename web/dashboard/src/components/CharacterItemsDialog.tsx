/**
 * A character's belongings, in a modal with two tabs: what they carry and
 * wear (Inventory), and what sits in their private warehouse.
 *
 * Read-only by design — the API it calls is read-only too, because live
 * inventories are memory-first in the game server. What this shows is the
 * last persisted state: exact for an offline character, autosave-fresh for an
 * online one.
 */
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { api, type CharacterItem } from "../lib/api";
import { ItemIcon } from "./ItemIcon";
import { Alert, Spinner, cx } from "./ui";

type Tab = "inventory" | "quest" | "warehouse" | "paperdoll";

const TABS: ReadonlyArray<{ key: Tab; label: string }> = [
  { key: "paperdoll", label: "Paperdoll" },
  { key: "inventory", label: "Inventory" },
  { key: "quest", label: "Quest" },
  { key: "warehouse", label: "Warehouse" },
];

export function CharacterItemsDialog({ name, onClose }: { name: string; onClose: () => void }) {
  const items = useQuery({
    queryKey: ["characterItems", name],
    queryFn: () => api.characterItems(name),
  });
  const [tab, setTab] = useState<Tab>("paperdoll");

  // A native <dialog>, shown modally: Escape, focus containment and the
  // top-layer backdrop all come from the platform instead of hand-rolled
  // listeners. The parent unmounts us on close, so `showModal` runs once.
  // While it is up, the page behind must not scroll. Two separate causes are
  // handled here:
  //  - `showModal()`'s focusing steps scroll the dialog's *pre-top-layer* DOM
  //    position (deep in a character list) into view, yanking the page to the
  //    top — undone by restoring the scroll offset synchronously, before the
  //    browser paints.
  //  - wheel/touch scrolling would still chain through the backdrop to the
  //    page — frozen by hiding overflow on the root element (not body, whose
  //    scroller would discard its offset — itself a jump-to-top).
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const { scrollX, scrollY } = window;
    ref.current?.showModal();
    window.scrollTo(scrollX, scrollY);
    const root = document.documentElement;
    const previous = root.style.overflow;
    root.style.overflow = "hidden";
    return () => {
      root.style.overflow = previous;
    };
  }, []);

  // The tabs are projections of one payload, split the way the game client
  // splits them: worn gear on the paperdoll, quest items on their own tab,
  // and the inventory holding only what remains.
  const rows = !items.data
    ? []
    : tab === "inventory"
      ? items.data.inventory.filter((i) => !i.equipped && !i.quest)
      : tab === "quest"
        ? items.data.inventory.filter((i) => i.quest)
        : tab === "warehouse"
          ? items.data.warehouse
          : [];
  const tabCount = (t: Tab): number | undefined => {
    if (!items.data) return undefined;
    if (t === "paperdoll") return items.data.inventory.filter((i) => i.equipped).length;
    if (t === "inventory") {
      return items.data.inventory.filter((i) => !i.equipped && !i.quest).length;
    }
    if (t === "quest") return items.data.inventory.filter((i) => i.quest).length;
    return items.data[t].length;
  };

  return (
    // The dialog element itself has no padding, so a click whose target is the
    // dialog (not a child) can only be on the backdrop area — the standard
    // click-away test for native dialogs.
    // biome-ignore lint/a11y/useKeyWithClickEvents: the keyboard path already exists — Escape closes a native modal dialog.
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      aria-label={`${name}'s items`}
      // A fixed height, not max-height: the two tabs rarely hold the same
      // number of rows, and a dialog sized to its content would change height
      // — and therefore jump, being centered — on every tab switch. The list
      // area scrolls inside instead.
      // `inset-0 m-auto` centers in the viewport — the position:fixed half of
      // that lives in globals.css as an unlayered `dialog:modal` rule, since
      // the `.fixed` utility cannot beat the UA's absolute positioning from
      // inside @layer utilities.
      // Width leaves a 1rem gutter each side on phones — `w-full` would pin
      // the rounded corners to the screen edges.
      className="glass-strong glass-sheen animate-rise inset-0 m-auto h-[min(37.5rem,85vh)]
                 w-[calc(100%-2rem)] max-w-lg overflow-hidden rounded-2xl bg-transparent p-0
                 text-(--text) backdrop:bg-black/60 backdrop:backdrop-blur-sm"
    >
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-between gap-3 border-b border-(--surface-border) px-5 py-3.5">
          <p className="truncate font-semibold">{name}</p>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="grid size-8 shrink-0 place-items-center rounded-lg text-(--text-faint)
                       transition-colors hover:bg-(--surface-strong) hover:text-(--text)"
          >
            <svg viewBox="0 0 20 20" aria-hidden className="size-4">
              <path
                d="m5.5 5.5 9 9m0-9-9 9"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>

        {/* Scrolls sideways where the four tabs outgrow a phone screen; the
            scrollbar itself stays hidden, as tab strips are swiped, not
            dragged by a bar. `shrink-0` is load-bearing: setting overflow
            makes a flex item's min-height collapse to 0, and a short viewport
            would otherwise crush the strip instead of the list below. */}
        <div
          className="flex shrink-0 gap-1 overflow-x-auto border-b border-(--surface-border) px-3
                     pt-2 scrollbar-none [&::-webkit-scrollbar]:hidden"
        >
          {TABS.map(({ key, label }) => (
            <button
              key={key}
              type="button"
              onClick={() => setTab(key)}
              aria-selected={tab === key}
              role="tab"
              className={cx(
                "shrink-0 rounded-t-lg px-4 py-2 text-sm font-medium whitespace-nowrap transition-colors",
                tab === key
                  ? "border-b-2 border-accent-400 text-(--text)"
                  : "text-(--text-faint) hover:text-(--text)",
              )}
            >
              {label}
              {tabCount(key) !== undefined && (
                <span className="ml-1.5 text-xs text-(--text-faint)">{tabCount(key)}</span>
              )}
            </button>
          ))}
        </div>

        {/* Keyed by tab so a switch remounts the pane: the rise-soft animation
            replays and the scroll position resets to the new list's top.
            Always a scroll container — on desktop heights the paperdoll fits
            without scrolling, but phone viewports need it to. Its tooltips
            stay unclipped because the pane reserves their headroom (see
            PaperdollPane's top padding). */}
        <div key={tab} className="animate-rise-soft flex-1 overflow-y-auto overscroll-contain">
          {items.isPending ? (
            <div className="flex items-center justify-center gap-3 p-8 text-sm text-(--text-muted)">
              <Spinner /> Loading items…
            </div>
          ) : items.isError ? (
            <div className="p-5">
              <Alert kind="error">Couldn't load this character's items. Try again.</Alert>
            </div>
          ) : tab === "paperdoll" ? (
            <PaperdollPane items={items.data.inventory} />
          ) : rows.length === 0 ? (
            <p className="p-8 text-center text-sm text-(--text-muted)">
              {tab === "inventory"
                ? "Nothing in the inventory."
                : tab === "quest"
                  ? "No quest items."
                  : "The warehouse is empty."}
            </p>
          ) : (
            <ul className="divide-y divide-(--surface-border)">
              {rows.map((item) => (
                <ItemRow key={item.objectId} item={item} />
              ))}
            </ul>
          )}
        </div>
      </div>
    </dialog>
  );
}

/**
 * Paperdoll slot ids, mirroring the server's `PaperdollSlot` numbering — the
 * `slot` field of an equipped item is exactly this value.
 */
const SLOT = {
  under: 0,
  head: 1,
  hair: 2,
  hair2: 3,
  neck: 4,
  rhand: 5,
  chest: 6,
  lhand: 7,
  rear: 8,
  lear: 9,
  gloves: 10,
  legs: 11,
  feet: 12,
  rfinger: 13,
  lfinger: 14,
  lbracelet: 15,
  rbracelet: 16,
  deco: [17, 18, 19, 20, 21, 22],
  cloak: 23,
  belt: 24,
  brooch: 25,
} as const;

/**
 * What an empty slot shows: a representative icon from the atlas, ghosted by
 * the slot renderer — the same trick the client's own window uses, so each
 * socket says what belongs in it without shipping any new artwork.
 */
const SLOT_GHOST: Record<number, string> = {
  [SLOT.under]: "branchicon.icon.shirt_of_explorer_i00",
  [SLOT.head]: "icon.armor_helmet_i00",
  [SLOT.hair]: "branchsys2.icon.freya_hair_accessary",
  [SLOT.hair2]: "branchsys2.icon.freya_hair_accessary",
  [SLOT.neck]: "icon.accessary_adamantite_necklace_i00",
  [SLOT.rhand]: "icon.weapon_small_sword_i00",
  [SLOT.chest]: "icon.armor_t01_u_i00",
  [SLOT.lhand]: "icon.shield_small_shield_i00",
  [SLOT.rear]: "icon.accessary_adamantite_earing_i00",
  [SLOT.lear]: "icon.accessary_adamantite_earing_i00",
  [SLOT.gloves]: "icon.armor_gauntlet_i00",
  [SLOT.legs]: "icon.armor_t01_l_i00",
  [SLOT.feet]: "icon.armor_iron_boots_i00",
  [SLOT.rfinger]: "icon.accessary_adamantite_ring_i00",
  [SLOT.lfinger]: "icon.accessary_adamantite_ring_i00",
  [SLOT.lbracelet]: "icon.dimension_bracelet_i00",
  [SLOT.rbracelet]: "icon.dimension_bracelet_i00",
  [SLOT.cloak]: "branchsys2.icon.br_zaken_cloak_i00",
  [SLOT.belt]: "icon.armor_belt_i00",
  [SLOT.brooch]: "icon.etc_bm_brooch_lavianrose_i00",
  [SLOT.deco[0]]: "icon.etc_talisman_i00",
};

/**
 * The equipment doll, laid out like the client's own window: head row over
 * armor rows, weapon and shield centered, jewelry below, then the talisman
 * strip and bracelets — in the site's surface colors rather than the
 * client's. Every slot renders, empty ones as a ghosted icon of what belongs
 * there, and the whole doll is sized to fit the pane without scrolling.
 */
function PaperdollPane({ items }: { items: CharacterItem[] }) {
  const bySlot = new Map<number, CharacterItem>();
  for (const item of items) {
    if (item.equipped && item.slot !== null) bySlot.set(item.slot, item);
  }
  // Which slot's tooltip a tap has pinned open. Hover needs no state — it is
  // pure CSS and therefore instant — but touch has no hover, so on mobile a
  // tap toggles the tooltip instead; tapping elsewhere on the pane clears it.
  const [pinned, setPinned] = useState<number | null>(null);
  const slot = (id: number, label: string, small = false) => (
    <PaperdollSlot
      item={bySlot.get(id)}
      label={label}
      small={small}
      ghost={SLOT_GHOST[id] ?? null}
      pinned={pinned === id}
      onToggle={() => setPinned(pinned === id ? null : id)}
    />
  );

  return (
    // The top padding is tooltip headroom: slot tooltips sit 2rem above their
    // slot, and the pane is a scroll container, so the first row's tooltip
    // must fit inside the content box or it gets clipped.
    //
    // biome-ignore lint/a11y/noStaticElementInteractions: clearing a tooltip on outside tap is pointer-only convenience; keyboard users move focus instead.
    // biome-ignore lint/a11y/useKeyWithClickEvents: same — there is no keyboard equivalent of "tap elsewhere"; Escape and focus changes already dismiss.
    <div
      className="flex flex-col items-center gap-3 px-4 pt-10 pb-4"
      onClick={() => setPinned(null)}
    >
      <div className="flex flex-col gap-2">
        <div className="flex justify-center gap-2">
          {slot(SLOT.hair, "Hair Accessory")}
          {slot(SLOT.head, "Helmet")}
          {slot(SLOT.hair2, "Hair Accessory 2")}
        </div>
        <div className="flex justify-center gap-2">
          {slot(SLOT.gloves, "Gloves")}
          {slot(SLOT.chest, "Upper Armor")}
          {slot(SLOT.feet, "Boots")}
        </div>
        <div className="flex justify-center gap-2">
          {slot(SLOT.cloak, "Cloak")}
          {slot(SLOT.legs, "Lower Armor")}
          {slot(SLOT.belt, "Belt")}
        </div>
        <div className="flex justify-center gap-2">
          {slot(SLOT.rhand, "Weapon")}
          {slot(SLOT.lhand, "Shield / Off-hand")}
        </div>
        <div className="flex justify-center gap-2">
          {slot(SLOT.lear, "Earring")}
          {slot(SLOT.rear, "Earring")}
          {slot(SLOT.neck, "Necklace")}
        </div>
        <div className="flex justify-center gap-2">
          {slot(SLOT.lfinger, "Ring")}
          {slot(SLOT.rfinger, "Ring")}
          {slot(SLOT.brooch, "Brooch")}
        </div>
      </div>

      {/* The talisman strip is its own framed band in the client; keep that. */}
      <div className="flex gap-1.5 rounded-xl border border-(--surface-border) p-1.5">
        {SLOT.deco.map((id, i) => (
          <PaperdollSlot
            key={id}
            item={bySlot.get(id)}
            label={`Talisman ${i + 1}`}
            ghost={SLOT_GHOST[SLOT.deco[0]] ?? null}
            pinned={pinned === id}
            onToggle={() => setPinned(pinned === id ? null : id)}
            small
          />
        ))}
      </div>

      {/* The bracelets, with the underwear slot tucked beside them — off the
          doll's silhouette but still present, and the doll fits the pane. */}
      <div className="flex items-center justify-center gap-2">
        {slot(SLOT.lbracelet, "Bracelet")}
        {slot(SLOT.rbracelet, "Bracelet")}
        {slot(SLOT.under, "Underwear", true)}
      </div>
    </div>
  );
}

function PaperdollSlot({
  item,
  label,
  ghost,
  pinned,
  onToggle,
  small,
}: {
  item: CharacterItem | undefined;
  label: string;
  /** Atlas reference ghosted into the empty socket to say what fits here. */
  ghost: string | null;
  /** A tap pinned this slot's tooltip open (the touch stand-in for hover). */
  pinned: boolean;
  onToggle: () => void;
  small?: boolean;
}) {
  const title = item ? `${item.enchant > 0 ? `+${item.enchant} ` : ""}${item.name}` : label;

  return (
    <button
      type="button"
      aria-label={title}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
      className={cx(
        "group relative grid shrink-0 place-items-center rounded-lg border transition-colors duration-200",
        small ? "size-9" : "size-11",
        item
          ? "border-(--surface-border) bg-(--surface-strong) hover:bg-brand-500/25"
          : "border-(--surface-border) bg-black/25",
      )}
    >
      {item ? (
        <ItemIcon
          bare
          icon={item.icon}
          className="transition-transform duration-200 ease-out-soft group-hover:scale-125"
        />
      ) : (
        ghost && <ItemIcon bare icon={ghost} className="opacity-25 grayscale" />
      )}
      {item && item.enchant > 0 && (
        <span
          className="absolute -top-1.5 -right-1.5 rounded bg-accent-400 px-1 text-[10px] font-bold
                     text-brand-950"
        >
          +{item.enchant}
        </span>
      )}
      {/* Not the native title tooltip: that waits about a second, and touch
          never sees it. This one fades in on hover with no delay, and a tap
          pins it (`pinned`) for pointerless screens. */}
      <span
        role="tooltip"
        className={cx(
          "pointer-events-none absolute -top-8 left-1/2 z-10 -translate-x-1/2 rounded-lg border",
          "border-(--surface-border) bg-brand-950/95 px-2 py-1 text-xs font-medium",
          "whitespace-nowrap text-white shadow-lg",
          "opacity-0 transition-opacity duration-100 group-hover:opacity-100",
          pinned && "opacity-100",
        )}
      >
        {item && item.enchant > 0 && <span className="text-accent-300">+{item.enchant} </span>}
        {item ? item.name : label}
      </span>
    </button>
  );
}

function ItemRow({ item }: { item: CharacterItem }) {
  return (
    <li
      className="group flex items-center gap-3 px-5 py-2.5 transition-colors duration-200
                 hover:bg-(--surface-strong)"
    >
      {/* The icon pops on row hover. Transform scales the sprite cleanly and
          never affects layout, so neighbouring rows stay put. */}
      <ItemIcon
        icon={item.icon}
        className={cx(
          "transition-transform duration-200 ease-out-soft group-hover:scale-125",
          item.equipped && "ring-2 ring-accent-400/70",
        )}
      />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">
          {item.enchant > 0 && (
            <span className="text-accent-500 dark:text-accent-300">+{item.enchant} </span>
          )}
          {item.name}
        </p>
        <p className="truncate text-xs text-(--text-faint)">
          {item.type}
          {item.equipped && " · Equipped"}
        </p>
      </div>
      {item.count > 1 && (
        <p className="shrink-0 text-sm tabular-nums text-(--text-muted)">
          ×{item.count.toLocaleString("en-US")}
        </p>
      )}
    </li>
  );
}
