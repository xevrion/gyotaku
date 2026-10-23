"use client";

// Every feature as a small working piece of the app, not a picture of it:
// each card's demo behaves the way gyotaku does (same rules, same keys), so
// poking it is the explanation. Facts follow docs/usage.md.
//
// The screenshots are real: small pages rendered like the apps they imitate,
// photographed with Chrome, and read by gyotaku's own OCR
// (scripts/demo-shots.ts and scripts/feature-shots.ts). Every search, filter
// and copy here runs over exactly the text gyotaku read, misreads included,
// and lights the boxes it found. All the data in them is made up.

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { AnimatePresence, MotionConfig, motion, useInView } from "motion/react";
import Image, { type StaticImageData } from "next/image";
import { Noto_Sans_Devanagari } from "next/font/google";
import { useReducedMotion } from "@/lib/use-reduced-motion";
import { DEMO_SHOTS } from "@/assets/demo";
import { SHOTS } from "@/assets/demo/features";
import redFuji from "@/assets/prints/red-fuji.jpg";
import suddenShower from "@/assets/prints/sudden-shower.jpg";

// Only the scripts card's chips need it, so it never blocks the first paint.
const devanagari = Noto_Sans_Devanagari({
  subsets: ["devanagari"],
  weight: ["400", "500"],
  preload: false,
});

const EASE = "cubic-bezier(0.23, 1, 0.32, 1)";
const SPRING = { type: "spring", duration: 0.3, bounce: 0 } as const;
const ICON_IN = { scale: 1, opacity: 1, filter: "blur(0px)" };
const ICON_OUT = { scale: 0.25, opacity: 0, filter: "blur(4px)" };


/* ------------------------------------------------------------------ */
/* Shared pieces                                                       */
/* ------------------------------------------------------------------ */

// One surface per card: the demo is a region of the card itself, divided
// from the words by a hairline, not a second box sunk inside it. No entrance
// animation; motion is kept for what the visitor does inside.
function Card({
  title,
  body,
  wide,
  height = 330,
  children,
}: {
  title: string;
  body: ReactNode;
  wide?: boolean;
  height?: number;
  children: ReactNode;
}) {
  // One row of the list under the showcases: what it does on the left, the
  // feature running beside it. A list rather than a grid of equal boxes, so
  // each one reads in turn and its demo gets the width to work in.
  return (
    <li className="grid grid-cols-[minmax(0,1fr)] gap-6 py-10 first:pt-0 last:pb-0 md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)] md:items-center md:gap-14 md:py-12">
      <div className="max-w-sm">
        <h3 className="text-[20px] leading-snug font-medium tracking-[-0.01em] text-ink">{title}</h3>
        <p className="mt-2 text-[15.5px] leading-relaxed text-dim">{body}</p>
      </div>
      <div
        data-card={title}
        className="@container relative overflow-hidden rounded-2xl bg-panel shadow-[var(--shadow)]"
        style={{ height: wide ? height - 30 : height, "--card-h": `${height}px` } as CSSProperties}
      >
        {children}
      </div>
    </li>
  );
}

// The big rows: the feature running live in an app window, floating on a
// woodblock print the way the hero's search floats on the Great Wave, with a
// short title and two plain lines beside it.
function Showcase({
  print,
  printAlt,
  crop,
  credit,
  title,
  body,
  hint,
  flip,
  height,
  zoom = 1,
  children,
}: {
  print: StaticImageData;
  printAlt: string;
  // Which part of the print shows: how far it's enlarged, and around where.
  crop: { scale: number; origin: string };
  credit: ReactNode;
  title: string;
  body: ReactNode;
  hint?: ReactNode;
  flip?: boolean;
  height: number;
  // Small demos are drawn larger here, so they fill the window like the
  // real app would; pointer math stays right because zoom scales layout.
  zoom?: number;
  children: ReactNode;
}) {
  return (
    <div
      data-row={title}
      // minmax(0, 1fr): on a phone the single column must not grow to fit a
      // wide demo, or the whole row slides off the right edge.
      className={`grid grid-cols-[minmax(0,1fr)] items-center gap-8 md:gap-14 ${
        flip
          ? "md:grid-cols-[minmax(0,1.2fr)_minmax(0,0.8fr)]"
          : "md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]"
      }`}
    >
      <div className={`max-w-md ${flip ? "md:order-2" : ""}`}>
        <h3 className="font-display text-[1.75rem] leading-[1.12] text-ink sm:text-[2.1rem]">{title}</h3>
        <p className="mt-3 text-[17px] leading-relaxed text-dim">{body}</p>
        {hint && <p className="mt-4 text-[13px] text-faint">{hint}</p>}
      </div>
      <figure className={flip ? "md:order-1" : ""}>
        <div className="relative overflow-hidden rounded-[22px] sm:rounded-[28px]">
          <Image
            src={print}
            alt={printAlt}
            fill
            placeholder="blur"
            sizes="(min-width: 1024px) 600px, 100vw"
            className="object-cover"
            style={{ scale: crop.scale, transformOrigin: crop.origin }}
          />
          <div className="relative p-3 sm:p-9">
            <div
              data-window={title}
              className="relative overflow-hidden rounded-xl bg-panel"
              style={{
                height,
                boxShadow: "0 30px 80px -20px rgb(0 0 0 / 0.55), 0 0 0 1px rgb(255 255 255 / 0.08)",
              }}
            >
              {/* Drawn larger only where the window has room for it; on a
                  phone the demo gets the window at its own size. The demo
                  inside reads its width as a container, so it can reflow. */}
              <div
                className="@container h-[var(--h)] md:h-[calc(var(--h)/var(--z))] md:[zoom:var(--z)]"
                style={{ "--z": zoom, "--h": `${height}px` } as CSSProperties}
              >
                {children}
              </div>
            </div>
          </div>
        </div>
        <figcaption className="mt-3 text-right text-[12px] text-faint">Background: {credit}</figcaption>
      </figure>
    </div>
  );
}

function Kbd({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <kbd
      className={`inline-flex h-5 min-w-5 items-center justify-center rounded-[5px] bg-panel px-1.5 font-mono text-[11px] leading-none text-dim shadow-[0_0_0_1px_var(--line),inset_0_-1px_0_var(--line)] ${className}`}
    >
      {children}
    </kbd>
  );
}

// Key hints only where there's a keyboard to press them on.
function KeyHint({ children }: { children: ReactNode }) {
  return (
    <span className="hidden [@media(hover:hover)_and_(pointer:fine)]:inline-flex">
      <Kbd className="h-4 min-w-4 px-1 text-[10px]">{children}</Kbd>
    </span>
  );
}

// A small no, for a refused input: a quick shake, done with the Web
// Animations API so it replays on every refusal.
function shake(el: Element | null) {
  if (!el || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  el.animate(
    [{ translate: "0" }, { translate: "-4px 0" }, { translate: "4px 0" }, { translate: "-2px 0" }, { translate: "0" }],
    { duration: 280, easing: EASE },
  );
}

function Code({ children }: { children: ReactNode }) {
  return <code className="rounded-[5px] bg-sunk px-1 py-px font-mono text-[13px] text-ink">{children}</code>;
}

function Toast({ show, children }: { show: boolean; children: ReactNode }) {
  return (
    <div
      role="status"
      aria-live="polite"
      className="pointer-events-none absolute inset-x-0 bottom-2.5 z-30 flex justify-center"
    >
      <div
        className="flex h-8 items-center gap-2 rounded-full bg-panel px-3 text-[13px] text-ink shadow-[var(--shadow)]"
        style={{
          opacity: show ? 1 : 0,
          transform: show ? "translateY(0)" : "translateY(6px)",
          transition: `opacity 200ms ${EASE}, transform 200ms ${EASE}`,
        }}
      >
        {show ? children : null}
      </div>
    </div>
  );
}

// A short-lived message: shows, then goes on its own.
function useFlash(ms = 1600) {
  const [text, setText] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flash = useCallback(
    (t: string) => {
      setText(t);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setText(null), ms);
    },
    [ms],
  );
  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);
  return [text, flash] as const;
}

function SearchIcon({ className = "size-3.5" }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" aria-hidden className={`shrink-0 ${className}`}>
      <circle cx="7" cy="7" r="4.25" />
      <path d="m10.25 10.25 3 3" />
    </svg>
  );
}

function CheckIcon({ className = "size-3.5" }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round" aria-hidden className={`shrink-0 ${className}`}>
      <path d="M3.5 8.5l3 3 6-7" />
    </svg>
  );
}

function FolderIcon({ className = "size-3.5" }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinejoin="round" aria-hidden className={`shrink-0 ${className}`}>
      <path d="M2 4.5c0-.83.67-1.5 1.5-1.5h2.88c.4 0 .78.16 1.06.44L8.5 4.5h4c.83 0 1.5.67 1.5 1.5v5.5c0 .83-.67 1.5-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5v-7Z" />
    </svg>
  );
}

// Whether the page is dark, following both the system and the theme toggle
// (which sets <html data-theme>). A pale screenshot faded on a dark panel
// still reads as a grey slab, so it recedes further there.
const DARK = "(prefers-color-scheme: dark)";
function subscribeTheme(onChange: () => void) {
  const media = matchMedia(DARK);
  const observer = new MutationObserver(onChange);
  media.addEventListener("change", onChange);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  return () => {
    media.removeEventListener("change", onChange);
    observer.disconnect();
  };
}
function useDark() {
  return useSyncExternalStore(
    subscribeTheme,
    () => {
      const t = document.documentElement.dataset.theme;
      return t ? t === "dark" : matchMedia(DARK).matches;
    },
    () => false,
  );
}

type OcrLine = { text: string; x: number; y: number; w: number; h: number; score: number };
type ShotData = { id: string; name: string; lines: OcrLine[]; image: StaticImageData };

// A real screenshot at its own proportions, with the boxes gyotaku read
// drawn over it in fractions of the picture, so a lit line lands on its words
// at any size. Solid for the text as read, dashed for a near match.
function Shot({
  shot,
  hits,
  sizes,
  className = "",
  eager = false,
}: {
  shot: ShotData;
  hits?: Hit[];
  sizes: string;
  className?: string;
  eager?: boolean;
}) {
  return (
    <div
      className={`relative w-full ${className}`}
      style={{ aspectRatio: `${shot.image.width} / ${shot.image.height}` }}
    >
      <Image
        src={shot.image}
        alt={`Screenshot of ${shot.name}`}
        fill
        placeholder="blur"
        loading={eager ? "eager" : "lazy"}
        sizes={sizes}
        draggable={false}
        className="object-cover select-none"
      />
      {hits &&
        shot.lines.map((line, i) => (
          <span
            key={i}
            aria-hidden
            className="absolute rounded-[2px] motion-reduce:transition-none"
            style={{
              left: `calc(${line.x * 100}% - 2px)`,
              top: `calc(${line.y * 100}% - 1px)`,
              width: `calc(${line.w * 100}% + 4px)`,
              height: `calc(${line.h * 100}% + 2px)`,
              opacity: hits[i] ? 1 : 0,
              scale: hits[i] ? 1 : 0.96,
              background: "rgb(255 116 56 / 0.22)",
              boxShadow: hits[i] === "near" ? "none" : "inset 0 0 0 1px var(--shu)",
              outline: hits[i] === "near" ? "1px dashed var(--shu)" : "none",
              outlineOffset: "-1px",
              transition: `opacity 200ms ${EASE}, scale 200ms ${EASE}`,
            }}
          />
        ))}
    </div>
  );
}

// How a screenshot tile sits in a set of results: a match stays exactly as
// bright as it was, ringed in shu with a sliver of window between; anything
// without the word recedes, faded and drained of color. Never a grey veil.
// `scale` is left out on motion elements, where Motion owns the transform.
function tileState(found: boolean, searching: boolean, dark: boolean, withScale = true): CSSProperties {
  const recede = searching && !found;
  return {
    opacity: recede ? (dark ? 0.12 : 0.32) : 1,
    filter: recede ? "grayscale(1)" : "grayscale(0)",
    ...(withScale ? { scale: recede ? 0.985 : 1 } : {}),
    boxShadow: found
      ? "0 0 0 2px var(--panel), 0 0 0 3.5px var(--shu)"
      : "0 0 0 2px transparent, 0 0 0 3.5px transparent",
    transition: `opacity 220ms ${EASE}, filter 220ms ${EASE}, scale 220ms ${EASE}, box-shadow 220ms ${EASE}`,
  };
}

/* ------------------------------------------------------------------ */
/* 1. Near matches                                                     */
/* ------------------------------------------------------------------ */

// The same look-alike folding the app's index does: case, 0 and o, l 1 i |,
// 5 and s, 8 and b, rn for m, vv for w, cl for d.
function fold(s: string) {
  return s
    .toLowerCase()
    .replace(/rn/g, "m")
    .replace(/vv/g, "w")
    .replace(/cl/g, "d")
    .replace(/0/g, "o")
    .replace(/[1|!i]/g, "l")
    .replace(/5/g, "s")
    .replace(/8/g, "b");
}

type Hit = "exact" | "near" | null;
function hitOf(line: string, q: string): Hit {
  if (!q) return null;
  if (line.toLowerCase().includes(q)) return "exact";
  // Words under four characters only ever match exactly.
  if (q.replace(/\s/g, "").length >= 4 && fold(line).includes(fold(q))) return "near";
  return null;
}

// The order confirmation's fine print says "Invoice #3907", and gyotaku's OCR
// really reads it as "Involce #3907" (an i for an l, the commonest misread
// there is), from a screenshot taken at 1x like an older laptop's. Nothing
// here is typed in by hand; the near match below is that real misread.
const NEAR_SHOTS = [SHOTS.nearMail, SHOTS.nearOrder, SHOTS.nearChat];

const RANK: Record<string, number> = { exact: 0, near: 1, none: 2 };

function NearDemo() {
  const [query, setQuery] = useState("invoice");
  const q = query.trim().toLowerCase();
  const dark = useDark();

  const shots = NEAR_SHOTS.map((s) => {
    const raw = s.lines.map((l) => hitOf(l.text, q));
    // As in the app, a screenshot with an exact hit shows only those.
    const exactHere = raw.includes("exact");
    const hits = raw.map((h) => (h === "near" && exactHere ? null : h));
    const best: Hit = exactHere ? "exact" : raw.includes("near") ? "near" : null;
    return { ...s, hits, best };
  });
  // Exact matches always come first, near ones after, the rest sink.
  const ordered = q
    ? [...shots].sort((a, b) => RANK[a.best ?? "none"] - RANK[b.best ?? "none"])
    : shots;
  const exact = shots.filter((s) => s.best === "exact").length;
  const near = shots.filter((s) => s.best === "near").length;

  return (
    <div className="flex h-full flex-col">
      <label className="flex h-11 shrink-0 items-center gap-2.5 border-b border-line px-3.5 text-dim">
        <SearchIcon />
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          spellCheck={false}
          autoComplete="off"
          aria-label="Search the three example screenshots"
          className="feat-input min-w-0 flex-1 bg-transparent text-base text-ink caret-[var(--shu)] outline-none placeholder:text-faint sm:text-[14px]"
          placeholder="search 3 screenshots"
        />
        <span className="shrink-0 text-[12px] tabular-nums" aria-live="polite">
          {!q ? "3 screenshots" : exact + near === 0 ? "none" : `${exact} exact${near ? `, ${near} near` : ""}`}
        </span>
      </label>

      {/* Three screenshots where the window is wide; on a phone the two that
          rank first, with room for their lines to wrap instead of cutting
          off the word that's lit. */}
      {/* Each tile hugs its screenshot, cut off at the window's edge if it's
          taller, never padded out with blank paper. */}
      <div className="grid min-h-0 flex-1 grid-cols-2 grid-rows-[minmax(0,1fr)] items-start gap-2.5 p-3 @min-[26rem]:grid-cols-3">
        {ordered.map((s, rank) => (
          <motion.div
            layout
            transition={SPRING}
            key={s.id}
            className={`relative max-h-full min-h-0 overflow-hidden rounded-[6px] bg-white outline outline-1 -outline-offset-1 outline-[var(--outline)] ${
              rank === 2 ? "hidden @min-[26rem]:block" : ""
            }`}
            style={tileState(s.best !== null, q !== "", dark, false)}
          >
            <Shot shot={s} hits={s.hits} sizes="(min-width: 1024px) 200px, 45vw" />
            <span
              className="absolute bottom-1.5 left-1.5 z-[25] rounded-full border border-black/10 bg-white px-1.5 py-0.5 text-[10px] text-[#6e6e73] sm:bottom-2 sm:left-2"
              style={{
                opacity: s.best === "near" ? 1 : 0,
                transform: s.best === "near" ? "scale(1)" : "scale(0.96)",
                transition: `opacity 200ms ${EASE}, transform 200ms ${EASE}`,
              }}
            >
              near match
            </span>
          </motion.div>
        ))}
      </div>

      <div
        data-scroll-x
        className="flex shrink-0 items-center gap-1.5 overflow-x-auto px-2.5 pb-2.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {["invoice", "inv0ice", "#4021", "gate"].map((c) => {
          const on = q === c;
          return (
            <button
              key={c}
              type="button"
              aria-pressed={on}
              onClick={() => setQuery(on ? "" : c)}
              className={`press h-7 shrink-0 rounded-full border px-2.5 font-mono text-[12px] ${
                on ? "border-transparent bg-shu-soft text-shu" : "border-line text-dim hover:text-ink"
              }`}
            >
              {c}
            </button>
          );
        })}
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 2. Filters                                                          */
/* ------------------------------------------------------------------ */

const DATES: Record<string, number> = { today: 0, yesterday: 1, week: 6 };

// The hero's screenshots again, each filed in a folder and taken some days
// ago, the two things the filters look at.
const FILED: { id: string; folder: string; age: number }[] = [
  { id: "terminal", folder: "Discord", age: 0 },
  { id: "otp", folder: "Discord", age: 1 },
  { id: "invoice", folder: "Screenshots", age: 0 },
  { id: "boarding", folder: "Screenshots", age: 1 },
  { id: "sheet", folder: "Downloads", age: 3 },
  { id: "notes", folder: "Screenshots", age: 9 },
];
const ROWS = FILED.map((f) => ({ ...f, shot: DEMO_SHOTS.find((s) => s.id === f.id)! }));

type Token = { text: string; filter: boolean };

function parseQuery(q: string) {
  const tokens: Token[] = [];
  const words: string[] = [];
  let folder: string | null = null;
  let within: [number, number] | null = null;
  for (const part of q.split(/(\s+)/)) {
    const m = /^(in|date):(.*)$/i.exec(part);
    if (!m) {
      tokens.push({ text: part, filter: false });
      if (part.trim()) words.push(part.toLowerCase());
      continue;
    }
    const [, key, value] = m;
    const v = value.toLowerCase();
    if (key.toLowerCase() === "in" && v) {
      folder = v;
      tokens.push({ text: part, filter: true });
    } else if (key.toLowerCase() === "date" && v in DATES) {
      within = v === "week" ? [0, 6] : [DATES[v], DATES[v]];
      tokens.push({ text: part, filter: true });
    } else {
      // Half typed, like date:yes on the way to yesterday: ignored, not
      // searched, so the results don't empty while it's typed.
      tokens.push({ text: part, filter: false });
    }
  }
  return { tokens, words, folder, within };
}

function FiltersDemo() {
  const [query, setQuery] = useState("error in:discord");
  const mirror = useRef<HTMLDivElement>(null);
  const { tokens, words, folder, within } = parseQuery(query);
  const dark = useDark();

  // Every word somewhere in what gyotaku read, not necessarily one line.
  const matches = ROWS.map(
    (r) =>
      words.every((w) => r.shot.lines.some((l) => l.text.toLowerCase().includes(w))) &&
      (!folder || r.folder.toLowerCase().startsWith(folder)) &&
      (!within || (r.age >= within[0] && r.age <= within[1])),
  );
  const count = matches.filter(Boolean).length;
  const searching = query.trim() !== "";

  const toggle = (chip: string) => {
    const key = chip.split(":")[0];
    const parts = query.split(/\s+/).filter(Boolean);
    const has = parts.includes(chip);
    const next = parts.filter((p) => !p.toLowerCase().startsWith(`${key}:`));
    if (!has) next.push(chip);
    setQuery(next.join(" "));
  };

  return (
    <div className="flex h-full flex-col">
      <label className="relative flex h-11 shrink-0 items-center gap-2.5 border-b border-line px-3.5 text-dim">
        <SearchIcon />
        <span className="relative min-w-0 flex-1">
          {/* The input's own text is invisible; this copy behind it draws
              the same words, with finished filters dimmed like the app. */}
          <div
            ref={mirror}
            aria-hidden
            className="pointer-events-none absolute inset-0 flex items-center overflow-hidden text-base whitespace-pre sm:text-[14px]"
          >
            {tokens.map((t, i) => (
              <span
                key={i}
                className={t.filter ? "text-faint" : "text-ink"}
                style={{ transition: `color 150ms ${EASE}` }}
              >
                {t.text}
              </span>
            ))}
          </div>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onScroll={(e) => {
              if (mirror.current) mirror.current.scrollLeft = e.currentTarget.scrollLeft;
            }}
            spellCheck={false}
            autoComplete="off"
            aria-label="Search with filters"
            className="feat-input relative w-full bg-transparent text-base text-transparent caret-[var(--shu)] outline-none selection:bg-shu-soft sm:text-[14px]"
          />
        </span>
        <span className="shrink-0 text-[12px] tabular-nums" aria-live="polite">
          {count} of {ROWS.length}
        </span>
      </label>

      {/* The results, as tiles like the app shows them, each captioned with
          the folder and day the filters read. */}
      <ul className="grid min-h-0 flex-1 grid-cols-3 content-center gap-x-2.5 gap-y-2 px-3 py-2.5">
        {ROWS.map((r, i) => {
          const hits = r.shot.lines.map((l) =>
            matches[i] && words.some((w) => l.text.toLowerCase().includes(w)) ? ("exact" as const) : null,
          );
          return (
            <li key={r.id} className="flex min-w-0 flex-col gap-1">
              <div
                className="overflow-hidden rounded-[5px] outline outline-1 -outline-offset-1 outline-[var(--outline)]"
                style={tileState(searching && matches[i], searching, dark)}
              >
                <Shot shot={r.shot} hits={hits} sizes="120px" />
              </div>
              <span
                // A caption that stepped back with its tile is out of the
                // results, so screen readers skip it too.
                aria-hidden={searching && !matches[i] ? true : undefined}
                className="truncate text-[11px]"
                // Steps back by color, not opacity, so it stays legible.
                style={{
                  color: !searching || matches[i] ? "var(--dim)" : "var(--faint)",
                  transition: `color 220ms ${EASE}`,
                }}
              >
                {r.folder} · {r.age === 0 ? "today" : `${r.age}d`}
              </span>
            </li>
          );
        })}
      </ul>

      <div
        data-scroll-x
        className="flex shrink-0 items-center gap-1.5 overflow-x-auto px-2.5 pb-2.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {["in:discord", "in:screenshots", "date:yesterday", "date:week"].map((c) => {
          const on = query.split(/\s+/).includes(c);
          return (
            <button
              key={c}
              type="button"
              aria-pressed={on}
              onClick={() => toggle(c)}
              className={`press h-7 shrink-0 rounded-full border px-2.5 font-mono text-[12px] ${
                on ? "border-transparent bg-shu-soft text-shu" : "border-line text-dim hover:text-ink"
              }`}
            >
              {c}
            </button>
          );
        })}
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 3. Bulk trash with undo                                             */
/* ------------------------------------------------------------------ */

// Four real one-time code screenshots: what "otp" finds.
const OTPS = [
  { id: 1, from: "HDFC Bank", shot: SHOTS.otp1 },
  { id: 2, from: "Swiggy", shot: SHOTS.otp2 },
  { id: 3, from: "Google", shot: SHOTS.otp3 },
  { id: 4, from: "Zomato", shot: SHOTS.otp4 },
];

function TrashDemo() {
  const [here, setHere] = useState(OTPS.map((o) => o.id));
  const [marked, setMarked] = useState<number[]>([]);
  const [confirming, setConfirming] = useState(false);
  const [moved, setMoved] = useState<number[] | null>(null);

  const toggle = (id: number) => {
    setConfirming(false);
    setMarked((m) => (m.includes(id) ? m.filter((x) => x !== id) : [...m, id]));
  };
  const markAll = () => {
    setConfirming(false);
    setMarked(here);
  };
  const trash = () => {
    if (!marked.length) return;
    if (!confirming) {
      // Like the app: always asked once, with the count.
      setConfirming(true);
      return;
    }
    setHere((h) => h.filter((id) => !marked.includes(id)));
    setMoved(marked);
    setMarked([]);
    setConfirming(false);
  };
  const undo = () => {
    if (!moved) return;
    setHere(OTPS.map((o) => o.id).filter((id) => here.includes(id) || moved.includes(id)));
    setMoved(null);
  };
  const escape = () => {
    if (confirming) setConfirming(false);
    else setMarked([]);
  };

  const onKey = (e: ReactKeyboardEvent) => {
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (mod && e.shiftKey && k === "a") markAll();
    else if (mod && (k === "delete" || k === "backspace")) trash();
    else if (k === "enter" && confirming) trash();
    else if (mod && k === "z") undo();
    else if (k === "escape") escape();
    else return;
    e.preventDefault();
  };

  const n = marked.length;

  return (
    <div
      role="group"
      aria-label="Example search for otp. Keys: Ctrl Shift A marks all, Ctrl Delete moves to the trash, Ctrl Z puts them back."
      tabIndex={0}
      onKeyDown={onKey}
      className="feat-group flex h-full flex-col"
    >
      <div className="flex h-11 shrink-0 items-center gap-2.5 border-b border-line px-3.5 text-dim">
        <SearchIcon />
        <span className="flex-1 text-[14px] text-ink">otp</span>
        <button
          type="button"
          onClick={markAll}
          disabled={!here.length}
          className="press flex h-7 items-center gap-1.5 rounded-full border border-line px-2.5 text-[12px] text-dim hover:text-ink disabled:opacity-40"
        >
          mark all
          <KeyHint>ctrl shift a</KeyHint>
        </button>
      </div>

      {/* The tiles sit in the space above the bar, so it never covers them. */}
      <div className="relative flex min-h-0 flex-1 items-center px-3 pt-3 pb-[60px]">
        {/* Two by two, so each message stays big enough to read. */}
        {/* Capped so a wide row doesn't grow the tiles into the search bar:
            their height follows their width. */}
        <ul className="mx-auto grid w-full max-w-[28rem] grid-cols-2 gap-2.5">
          <AnimatePresence mode="popLayout" initial={false}>
            {OTPS.filter((o) => here.includes(o.id)).map((o) => {
              const on = marked.includes(o.id);
              return (
                <motion.li
                  key={o.id}
                  layout
                  initial={{ opacity: 0, scale: 0.96 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0, scale: 0.96, y: 4, transition: { duration: 0.18 } }}
                  transition={SPRING}
                >
                  <button
                    type="button"
                    aria-pressed={on}
                    aria-label={`${o.from} one-time code`}
                    onClick={() => toggle(o.id)}
                    // Cropped from the top: the sender and the message, without
                    // the empty thread below them.
                    className="press relative block aspect-[16/10] w-full overflow-hidden rounded-[6px] bg-white"
                    style={{
                      // A hairline edge always; marked adds the shu ring
                      // outside it, with a sliver of card between.
                      boxShadow: on
                        ? "0 0 0 1px var(--outline), 0 0 0 3px var(--panel), 0 0 0 4.5px var(--shu)"
                        : "0 0 0 1px var(--outline), 0 0 0 3px transparent, 0 0 0 4.5px transparent",
                      transition: `box-shadow 150ms ${EASE}, scale 160ms ${EASE}`,
                    }}
                  >
                    <Shot shot={o.shot} sizes="180px" />
                    <motion.span
                      aria-hidden
                      className="absolute top-1.5 right-1.5 grid size-4 place-items-center rounded-full bg-shu text-[var(--on-shu,#fff)]"
                      initial={false}
                      animate={on ? ICON_IN : ICON_OUT}
                      transition={SPRING}
                    >
                      <CheckIcon className="size-2.5" />
                    </motion.span>
                  </button>
                </motion.li>
              );
            })}
          </AnimatePresence>
        </ul>
        {!here.length && (
          <p className="absolute inset-0 grid place-items-center text-[13px] text-faint">nothing left, all in the trash</p>
        )}

        {/* The mark bar, and the confirmation it turns into. */}
        <div
          className="absolute inset-x-2.5 bottom-2.5 z-20 flex h-10 items-center gap-2 rounded-[8px] bg-panel pr-1.5 pl-3 text-[13px] shadow-[var(--shadow)]"
          style={{
            opacity: n ? 1 : 0,
            transform: n ? "translateY(0)" : "translateY(8px)",
            pointerEvents: n ? "auto" : "none",
            transition: `opacity 200ms ${EASE}, transform 200ms ${EASE}`,
          }}
        >
          <span className="flex-1 text-ink tabular-nums">
            {confirming ? `move ${n} to the trash?` : `${n} marked`}
          </span>
          <button
            type="button"
            onClick={trash}
            className="press flex h-7 items-center gap-1.5 rounded-[6px] px-2.5 text-[12px] font-medium"
            style={{
              background: confirming ? "var(--ink)" : "var(--sunk)",
              color: confirming ? "var(--bg)" : "var(--ink)",
              transition: `background-color 150ms ${EASE}, color 150ms ${EASE}, scale 160ms ${EASE}`,
            }}
          >
            {confirming ? "move" : "to the trash"}
            <span className="hidden font-mono text-[10px] opacity-70 [@media(hover:hover)_and_(pointer:fine)]:inline">
              {confirming ? "enter" : "ctrl del"}
            </span>
          </button>
        </div>

        <div
          role="status"
          aria-live="polite"
          className="absolute inset-x-2.5 bottom-2.5 z-10 flex h-10 items-center gap-2 rounded-[8px] bg-panel pr-1.5 pl-3 text-[13px] shadow-[var(--shadow)]"
          style={{
            opacity: moved && !n ? 1 : 0,
            transform: moved && !n ? "translateY(0)" : "translateY(8px)",
            pointerEvents: moved && !n ? "auto" : "none",
            transition: `opacity 200ms ${EASE}, transform 200ms ${EASE}`,
          }}
        >
          <span className="flex-1 text-ink">{moved ? `moved ${moved.length} to the trash` : ""}</span>
          <button
            type="button"
            onClick={undo}
            className="press flex h-7 items-center gap-1.5 rounded-[6px] bg-sunk px-2.5 text-[12px] font-medium text-ink"
          >
            undo
            <span className="hidden font-mono text-[10px] text-dim [@media(hover:hover)_and_(pointer:fine)]:inline">ctrl z</span>
          </button>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 4. Similar screenshots fold into one                                */
/* ------------------------------------------------------------------ */

// Four real shots of one group chat, taken while scrolling.
const BURST_SHOTS = [SHOTS.burst1, SHOTS.burst2, SHOTS.burst3, SHOTS.burst4];
const BURST = BURST_SHOTS.length;
const W = 160;
const H = Math.round((W * SHOTS.burst1.image.height) / SHOTS.burst1.image.width);
const STEP = 178;
// Room kept clear at each side of the unfolded row.
const SIDE = 16;

// Folded: stacked behind the newest, each a little askew. Unfolded: in a
// row, the rest right after it, overlapping a little where the window is too
// narrow to lay them side by side.
function burstPose(i: number, open: boolean, step: number) {
  if (open) return { x: (i - (BURST - 1) / 2) * step - W / 2, y: 0, rotate: 0 };
  const tilt = [0, -4, 3.5, -2][i];
  return { x: -W / 2 + i * 7, y: -i * 6, rotate: tilt };
}

function BurstDemo() {
  const [open, setOpen] = useState(false);
  const [touched, setTouched] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const seen = useInView(root, { once: true, amount: 0.7 });
  const reduce = useReducedMotion();
  const [step, setStep] = useState(STEP);

  // The unfolded row has to fit the window it's in. ResizeObserver reports
  // once on observe, so it only acts on a real change of width.
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    let last = -1;
    const fit = () => {
      const w = el.clientWidth;
      if (w === last) return;
      last = w;
      setStep(Math.min(STEP, Math.max(0, (w - 2 * SIDE - W) / (BURST - 1))));
    };
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Shown once, unprompted: it unfolds, holds, folds back. Touching the card
  // first skips it.
  useEffect(() => {
    if (!seen || reduce || touched) return;
    const a = setTimeout(() => setOpen(true), 900);
    const b = setTimeout(() => setOpen(false), 2700);
    return () => {
      clearTimeout(a);
      clearTimeout(b);
    };
  }, [seen, reduce, touched]);

  const flip = () => {
    setTouched(true);
    setOpen((o) => !o);
  };

  return (
    <div
      ref={root}
      role="group"
      aria-label="Four similar screenshots. Ctrl E shows or hides them."
      tabIndex={0}
      onPointerDown={() => setTouched(true)}
      onKeyDown={(e) => {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "e") {
          e.preventDefault();
          flip();
        }
      }}
      className="feat-group relative h-full"
    >
      <div className="absolute top-1/2 left-1/2" style={{ marginTop: -H / 2 - 22 }}>
        {Array.from({ length: BURST }, (_, i) => BURST - 1 - i).map((i) => (
          <motion.div
            key={i}
            className="absolute top-0 left-0 overflow-hidden rounded-[8px] bg-white shadow-[0_0_0_1px_rgb(0_0_0/0.06),0_2px_4px_rgb(0_0_0/0.1),0_12px_28px_-10px_rgb(0_0_0/0.35)]"
            style={{ width: W, height: H, zIndex: BURST - i }}
            initial={false}
            animate={burstPose(i, open, step)}
            transition={{ type: "spring", duration: 0.45, bounce: 0, delay: open ? i * 0.03 : (BURST - 1 - i) * 0.02 }}
          >
            <Shot shot={BURST_SHOTS[i]} sizes="160px" eager={i === 0} />
          </motion.div>
        ))}
      </div>

      <div className="absolute inset-x-0 bottom-3 flex justify-center">
        <button
          type="button"
          onClick={flip}
          aria-expanded={open}
          className="press relative flex h-7 items-center gap-1.5 rounded-full bg-panel px-3 text-[12px] text-ink shadow-[0_0_0_1px_var(--line)]"
        >
          <span className="relative grid">
            <motion.span
              className="col-start-1 row-start-1 whitespace-nowrap"
              initial={false}
              animate={open ? { opacity: 0, filter: "blur(4px)" } : { opacity: 1, filter: "blur(0px)" }}
              transition={SPRING}
            >
              +{BURST - 1} similar
            </motion.span>
            <motion.span
              className="col-start-1 row-start-1 whitespace-nowrap"
              initial={false}
              animate={open ? { opacity: 1, filter: "blur(0px)" } : { opacity: 0, filter: "blur(4px)" }}
              transition={SPRING}
            >
              hide {BURST - 1}
            </motion.span>
          </span>
          <KeyHint>ctrl e</KeyHint>
        </button>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 5. Copy just the part you need                                      */
/* ------------------------------------------------------------------ */

// A real booking confirmation and the lines gyotaku read from it, each with
// its box. A drag picks every line its box touches, like the app's open view.
const BOOKING = SHOTS.booking;

type Box = { x: number; y: number; w: number; h: number };

function CopyDemo() {
  const shot = useRef<HTMLDivElement>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  // Set once a press turns into a drag, so the click that ends it doesn't
  // also copy the single line it ended on.
  const dragged = useRef(false);
  const [box, setBox] = useState<Box | null>(null);
  const [picked, setPicked] = useState<number[]>([]);
  const [message, flash] = useFlash();

  const copy = (idx: number[]) => {
    const text = idx.map((i) => BOOKING.lines[i].text).join("\n");
    navigator.clipboard?.writeText(text).catch(() => {});
    flash(idx.length === 1 ? "copied 1 line" : `copied ${idx.length} lines`);
  };

  // The drag box against each OCR box, both in fractions of the picture.
  const within = (b: Box) => {
    const r = shot.current?.getBoundingClientRect();
    if (!r?.width) return [];
    const f = { x: b.x / r.width, y: b.y / r.height, w: b.w / r.width, h: b.h / r.height };
    return BOOKING.lines.flatMap((l, i) =>
      l.x < f.x + f.w && l.x + l.w > f.x && l.y < f.y + f.h && l.y + l.h > f.y ? [i] : [],
    );
  };

  // Measured on screen. Inside a zoomed showcase the screen is larger than
  // the layout, so the box is drawn divided by that zoom.
  const point = (e: ReactPointerEvent) => {
    const r = shot.current!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const zoomOf = () => {
    const el = shot.current;
    return el && el.offsetWidth ? el.getBoundingClientRect().width / el.offsetWidth : 1;
  };

  // Mouse and pen drag a box; on touch the page has to keep scrolling, so a
  // tap on a line copies that line instead.
  const down = (e: ReactPointerEvent) => {
    if (e.pointerType === "touch" || e.button !== 0) return;
    dragged.current = false;
    start.current = point(e);
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const move = (e: ReactPointerEvent) => {
    if (!start.current) return;
    const p = point(e);
    const b = {
      x: Math.min(p.x, start.current.x),
      y: Math.min(p.y, start.current.y),
      w: Math.abs(p.x - start.current.x),
      h: Math.abs(p.y - start.current.y),
    };
    if (b.w < 4 && b.h < 4) return;
    dragged.current = true;
    setBox(b);
    setPicked(within(b));
  };
  const up = () => {
    const b = box;
    start.current = null;
    if (b && picked.length) copy(picked);
    setBox(null);
    setPicked([]);
  };

  return (
    <div className="grid h-full place-items-center p-3 sm:p-4">
      <div
        ref={shot}
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
        // As large as the window allows at the picture's own proportions.
        className="group relative w-full cursor-crosshair overflow-hidden rounded-[8px] bg-white outline outline-1 -outline-offset-1 outline-[var(--outline)] select-none"
        style={{
          maxWidth: `min(100%, calc((var(--h) - 32px) * ${BOOKING.image.width / BOOKING.image.height}))`,
        }}
      >
        <Shot
          shot={BOOKING}
          hits={BOOKING.lines.map((_, i) => (picked.includes(i) ? "exact" : null))}
          sizes="(min-width: 1024px) 520px, 90vw"
        />
        {/* Each line gyotaku read is also a button over its own box: a click
            or a tap copies just that line, and a hover shows where it is. */}
        {BOOKING.lines.map((l, i) => (
          <button
            key={i}
            type="button"
            aria-label={`Copy "${l.text}"`}
            onClick={() => {
              if (dragged.current) {
                dragged.current = false;
                return;
              }
              copy([i]);
            }}
            className="absolute rounded-[2px] [@media(hover:hover)_and_(pointer:fine)]:hover:shadow-[inset_0_0_0_1px_rgb(0_0_0/0.18)]"
            style={{
              left: `calc(${l.x * 100}% - 2px)`,
              top: `calc(${l.y * 100}% - 1px)`,
              width: `calc(${l.w * 100}% + 4px)`,
              height: `calc(${l.h * 100}% + 2px)`,
              transition: `box-shadow 120ms ${EASE}`,
            }}
          />
        ))}
        {box && (
          <span
            aria-hidden
            className="pointer-events-none absolute rounded-[2px]"
            // The rubber band is drawn in the screenshot's own ink, so only
            // the lines it catches turn shu.
            style={{
              left: box.x / zoomOf(),
              top: box.y / zoomOf(),
              width: box.w / zoomOf(),
              height: box.h / zoomOf(),
              border: "1px dashed rgb(28 28 30 / 0.45)",
              background: "rgb(28 28 30 / 0.04)",
            }}
          />
        )}
      </div>
      <Toast show={!!message}>
        <motion.span initial={ICON_OUT} animate={ICON_IN} transition={SPRING} className="text-ink">
          <CheckIcon />
        </motion.span>
        {message}
      </Toast>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 6. Copied images become searchable                                  */
/* ------------------------------------------------------------------ */

function stamp() {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `Clipboard ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}.png`;
}

// A real picture someone copies from a chat and never saves, and what
// gyotaku reads from it once it lands in the folder.
const COPIED = SHOTS.copied;
const COPIED_TEXT = COPIED.lines.map((l) => l.text).join(" ");

function ClipboardDemo() {
  const [on, setOn] = useState(true);
  const [saved, setSaved] = useState(12);
  const [last, setLast] = useState<string | null>(null);
  type Flight = { id: number; left: number; top: number; dx: number; dy: number };
  const [flights, setFlights] = useState<Flight[]>([]);
  const stage = useRef<HTMLDivElement>(null);
  const source = useRef<HTMLDivElement>(null);
  const target = useRef<HTMLDivElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  const reduce = useReducedMotion();

  // A copy of the picture travels from where it was copied into the folder.
  const copyImage = () => {
    if (!on) {
      shake(toggle.current);
      return;
    }
    const s = stage.current?.getBoundingClientRect();
    const a = source.current?.getBoundingClientRect();
    const b = target.current?.getBoundingClientRect();
    if (!s || !a || !b || reduce) {
      land();
      return;
    }
    setFlights((f) => [
      ...f,
      {
        id: performance.now(),
        left: a.left - s.left,
        top: a.top - s.top,
        dx: b.left + b.width / 2 - (a.left + a.width / 2),
        dy: b.top + b.height / 2 - (a.top + a.height / 2),
      },
    ]);
  };
  const land = () => {
    setSaved((n) => n + 1);
    setLast(stamp());
  };

  return (
    <div className="flex h-full flex-col gap-2 p-3">
      {/* The settings row it's turned on from. */}
      <div className="flex items-center justify-between gap-3 rounded-[8px] bg-sunk px-3 py-2">
        <span className="flex min-w-0 flex-col">
          <span className="text-[13px] text-ink">save copied images</span>
          <span className="truncate text-[11px] text-faint">so pictures you only copied are searchable too</span>
        </span>
        {/* 44px wide hit area around a 36px switch. */}
        <button
          ref={toggle}
          type="button"
          role="switch"
          aria-checked={on}
          aria-label="Save copied images"
          onClick={() => setOn((o) => !o)}
          className="group -m-2 p-2"
        >
          <span
            className="relative block h-5 w-9 rounded-full"
            style={{ background: on ? "var(--ink)" : "var(--line)", transition: `background-color 150ms ${EASE}` }}
          >
            <span
              className="absolute top-0.5 left-0.5 size-4 rounded-full bg-[var(--panel)] shadow-[0_1px_2px_rgb(0_0_0/0.25)]"
              style={{
                transform: on ? "translateX(16px)" : "translateX(0)",
                transition: `transform 200ms ${EASE}`,
              }}
            />
          </span>
        </button>
      </div>

      {/* Side by side where the card is wide enough; stacked on a narrow
          card (the picture beside its button, the folder below), since the
          three pieces need about 340px in a row. */}
      <div
        ref={stage}
        className="relative flex flex-1 flex-col items-center justify-center gap-2 @min-[22rem]:flex-row @min-[22rem]:gap-3"
      >
        {/* Somewhere else on the screen: a picture someone copies. */}
        <div className="flex items-center gap-3 @min-[22rem]:flex-1 @min-[22rem]:flex-col @min-[22rem]:gap-2">
          <div ref={source} className="relative w-[112px] overflow-hidden rounded-[6px] outline outline-1 -outline-offset-1 outline-[var(--outline)] @min-[22rem]:w-[132px]">
            <Shot shot={COPIED} sizes="132px" />
          </div>
          <button
            type="button"
            onClick={copyImage}
            className="press flex h-7 items-center gap-1.5 rounded-full border border-line bg-panel px-2.5 text-[12px] whitespace-nowrap text-ink"
          >
            copy image
            <KeyHint>ctrl c</KeyHint>
          </button>
        </div>

        <svg viewBox="0 0 24 8" aria-hidden className="my-1 w-5 shrink-0 rotate-90 text-faint @min-[22rem]:my-0 @min-[22rem]:w-6 @min-[22rem]:rotate-0" fill="none" stroke="currentColor" strokeWidth={1.25} strokeLinecap="round" strokeLinejoin="round">
          <path d="M1 4h21M18.5 1 22 4l-3.5 3" />
        </svg>

        <div className="flex flex-col items-center gap-1.5 @min-[22rem]:flex-1 @min-[22rem]:gap-2">
          <div
            ref={target}
            className="flex h-10 items-center gap-2 rounded-[8px] bg-panel px-3 text-[13px] text-ink shadow-[0_0_0_1px_var(--line)]"
          >
            <FolderIcon className="size-4 text-dim" />
            Clipboard
            <span className="relative inline-flex overflow-hidden rounded-full bg-sunk px-1.5 text-[11px] text-dim tabular-nums">
              <AnimatePresence mode="popLayout" initial={false}>
                <motion.span
                  key={saved}
                  initial={{ y: 8, opacity: 0 }}
                  animate={{ y: 0, opacity: 1 }}
                  exit={{ y: -8, opacity: 0 }}
                  transition={{ duration: 0.22, ease: [0.23, 1, 0.32, 1] }}
                >
                  {saved}
                </motion.span>
              </AnimatePresence>
            </span>
          </div>
          <span className="font-mono text-[10px] text-faint">Pictures/Clipboard</span>
        </div>

        {flights.map((f) => (
          <motion.div
            key={f.id}
            aria-hidden
            className="pointer-events-none absolute z-20 w-[112px] overflow-hidden rounded-[6px] shadow-[0_8px_20px_-6px_rgb(0_0_0/0.35)] @min-[22rem]:w-[132px]"
            style={{ left: f.left, top: f.top }}
            initial={{ x: 0, y: 0, scale: 1, opacity: 1 }}
            animate={{ x: f.dx, y: f.dy, scale: 0.25, opacity: [1, 1, 0] }}
            transition={{ duration: 0.55, ease: [0.77, 0, 0.175, 1] }}
            onAnimationComplete={() => {
              setFlights((all) => all.filter((x) => x.id !== f.id));
              land();
            }}
          >
            <Shot shot={COPIED} sizes="132px" />
          </motion.div>
        ))}
      </div>

      <p className="flex h-5 items-center justify-center gap-1.5 text-[12px]" aria-live="polite">
        {!on ? (
          <span className="text-faint">off: copied images aren&apos;t kept</span>
        ) : last ? (
          <motion.span
            key={last}
            initial={{ opacity: 0, filter: "blur(4px)", y: 4 }}
            animate={{ opacity: 1, filter: "blur(0px)", y: 0 }}
            transition={{ duration: 0.25, ease: [0.23, 1, 0.32, 1] }}
            className="flex min-w-0 items-center gap-1.5"
            title={last}
          >
            <span className="size-1.5 shrink-0 rounded-full bg-shu" />
            {/* What gyotaku read from the saved picture: now findable. */}
            <span className="truncate text-dim">&ldquo;{COPIED_TEXT}&rdquo;</span>
            <span className="shrink-0 text-ink">searchable</span>
          </motion.span>
        ) : (
          <span className="text-faint">copy it, it gets saved and read</span>
        )}
      </p>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 7. Devanagari and vertical text                                     */
/* ------------------------------------------------------------------ */

// A Hindi order update with a vertical Japanese label beside it, read with
// Devanagari turned on. gyotaku's reading of the title drops a letter
// ("ऑ्डर" for "ऑर्डर"), kept as read; the words searched for here read cleanly.
const SCRIPTS = SHOTS.scripts;

function ScriptsDemo() {
  const [q, setQ] = useState<string | null>("स्टेटस");
  const hits = SCRIPTS.lines.map((l) => (q && l.text.includes(q) ? ("exact" as const) : null));
  const any = hits.some(Boolean);

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-11 shrink-0 items-center gap-2.5 border-b border-line px-3.5 text-dim">
        <SearchIcon />
        <span className={`flex-1 truncate text-[15px] text-ink ${devanagari.className}`}>
          {q ?? <span className="text-faint">pick a word</span>}
        </span>
      </div>

      <div className="grid min-h-0 flex-1 place-items-center p-3">
        <div
          className="w-full overflow-hidden rounded-[6px] bg-white outline outline-1 -outline-offset-1 outline-[var(--outline)]"
          style={{ maxWidth: `calc((var(--card-h) - 112px) * ${SCRIPTS.image.width / SCRIPTS.image.height})` }}
        >
          <Shot shot={SCRIPTS} hits={hits} sizes="(min-width: 1024px) 360px, 90vw" />
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1.5 px-2.5 pb-2.5" aria-live="polite">
        {["स्टेटस", "जोधपुर", "縦書き"].map((c) => {
          const pressed = q === c;
          return (
            <button
              key={c}
              type="button"
              aria-pressed={pressed}
              onClick={() => setQ(pressed ? null : c)}
              className={`press h-7 shrink-0 rounded-full border px-2.5 text-[13px] ${devanagari.className} ${
                pressed ? "border-transparent bg-shu-soft text-shu" : "border-line text-dim hover:text-ink"
              }`}
            >
              {c}
            </button>
          );
        })}
        <span className="ml-auto text-[12px] text-dim">{q ? (any ? "found" : "none") : ""}</span>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 8. Rebind any shortcut                                              */
/* ------------------------------------------------------------------ */

// The app's defaults (docs/usage.md). Three show on a phone, all six where
// the card has room for two columns.
const COMMANDS = [
  { id: "trash", name: "move to the trash", keys: ["ctrl", "delete"] },
  { id: "all", name: "mark every result", keys: ["ctrl", "shift", "a"] },
  { id: "similar", name: "show similar", keys: ["ctrl", "e"] },
  { id: "text", name: "copy the text", keys: ["ctrl", "c"] },
  { id: "image", name: "copy the image", keys: ["ctrl", "shift", "c"] },
  { id: "settings", name: "open settings", keys: ["ctrl", ","] },
];

const MODS = ["Control", "Shift", "Alt", "Meta"];

function keyName(k: string) {
  if (k === " ") return "space";
  if (k.startsWith("Arrow")) return k.slice(5).toLowerCase();
  return k.length === 1 ? k.toLowerCase() : k.toLowerCase();
}

function ShortcutsDemo() {
  const [keys, setKeys] = useState(COMMANDS.map((c) => c.keys));
  const [recording, setRecording] = useState<number | null>(null);
  const [held, setHeld] = useState<string[]>([]);
  const [error, setError] = useState<{ row: number; text: string } | null>(null);
  const [fresh, setFresh] = useState<number | null>(null);
  const rows = useRef<(HTMLButtonElement | null)[]>([]);

  const stop = () => {
    setRecording(null);
    setHeld([]);
  };

  const onKey = (i: number, e: ReactKeyboardEvent) => {
    if (recording !== i) return;
    if (e.key === "Tab") return;
    e.preventDefault();
    if (e.key === "Escape") return stop();
    const mods = [e.ctrlKey && "ctrl", e.altKey && "alt", e.shiftKey && "shift", e.metaKey && "super"].filter(Boolean) as string[];
    if (MODS.includes(e.key)) {
      setHeld(mods);
      return;
    }
    if (!mods.length && (e.key === "Delete" || e.key === "Backspace")) {
      setKeys((k) => k.map((v, j) => (j === i ? COMMANDS[i].keys : v)));
      setFresh(i);
      return stop();
    }
    const combo = [...mods, keyName(e.key)];
    const fn = /^f\d{1,2}$/.test(keyName(e.key));
    if (!fn && !mods.some((m) => m !== "shift")) {
      setError({ row: i, text: "needs ctrl, alt or super" });
      shake(rows.current[i]);
      return;
    }
    const clash = keys.findIndex((v, j) => j !== i && v.join("+") === combo.join("+"));
    if (clash >= 0) {
      setError({ row: i, text: `already used by ${COMMANDS[clash].name}` });
      shake(rows.current[i]);
      return;
    }
    setKeys((k) => k.map((v, j) => (j === i ? combo : v)));
    setError(null);
    setFresh(i);
    stop();
  };

  useEffect(() => {
    if (fresh === null) return;
    const t = setTimeout(() => setFresh(null), 900);
    return () => clearTimeout(t);
  }, [fresh]);

  return (
    // A slice of the settings page: rows of a name and its keys, the keys in
    // one right-aligned column, a hairline between rows.
    <div className="flex h-full flex-col justify-center px-3 py-3 @min-[22rem]:px-5">
      <p className="px-3 pb-2 text-[11px] font-medium tracking-wide text-faint">SHORTCUTS</p>
      <span id="shortcut-how" className="sr-only">
        Press Enter, then the new keys.
      </span>
      <div className="grid grid-cols-1 gap-x-8 @min-[34rem]:grid-flow-col @min-[34rem]:grid-cols-2 @min-[34rem]:grid-rows-3">
      {COMMANDS.map((c, i) => {
        const rec = recording === i;
        const err = error?.row === i ? error : null;
        return (
          <button
            key={c.id}
            ref={(el) => {
              rows.current[i] = el;
            }}
            type="button"
            onClick={() => {
              setError(null);
              setHeld([]);
              setRecording(rec ? null : i);
            }}
            onKeyDown={(e) => onKey(i, e)}
            onKeyUp={(e) => {
              if (rec && MODS.includes(e.key)) {
                setHeld([e.ctrlKey && "ctrl", e.altKey && "alt", e.shiftKey && "shift", e.metaKey && "super"].filter(Boolean) as string[]);
              }
            }}
            onBlur={() => rec && stop()}
            // The name comes from what's on the button (command and keys);
            // how to change it is said as a description.
            aria-describedby="shortcut-how"
            className={`press relative h-14 shrink-0 items-center gap-3 rounded-[8px] px-3 text-left after:absolute after:inset-x-3 after:bottom-0 after:h-px after:bg-line [&:nth-child(3n)]:after:hidden ${
              i >= 3 ? "hidden @min-[34rem]:flex" : "flex"
            }`}
            style={{
              background: rec ? "var(--sunk)" : "transparent",
              boxShadow: rec ? "0 0 0 1.5px var(--shu)" : "0 0 0 1.5px transparent",
              transition: `background-color 150ms ${EASE}, box-shadow 150ms ${EASE}, scale 160ms ${EASE}`,
            }}
          >
            <span className="flex min-w-0 flex-1 flex-col">
              <span className="truncate text-[13px] text-ink @min-[22rem]:text-[14px]">{c.name}</span>
              {/* Only there while recording or refusing, so at rest the name
                  sits level with its keys. A refusal is said plainly in ink,
                  with the shake; shu is kept for what's being recorded. */}
              {(err || rec) && (
                <span className="truncate text-[11px]" style={{ color: err ? "var(--ink)" : "var(--faint)" }}>
                  {err ? err.text : "escape cancels, delete resets"}
                </span>
              )}
            </span>
            <span className="relative grid shrink-0 justify-items-end">
              <motion.span
                className="col-start-1 row-start-1 flex items-center gap-0.5 @min-[22rem]:gap-1"
                initial={false}
                animate={rec ? { opacity: 0, filter: "blur(4px)" } : { opacity: 1, filter: "blur(0px)" }}
                transition={SPRING}
              >
                {keys[i].map((k) => (
                  <Kbd
                    key={k}
                    className={fresh === i ? "text-shu shadow-[0_0_0_1px_var(--shu)]" : ""}
                  >
                    {k}
                  </Kbd>
                ))}
              </motion.span>
              <motion.span
                className="col-start-1 row-start-1 flex items-center gap-1 text-[12px] text-shu"
                initial={false}
                animate={rec ? { opacity: 1, filter: "blur(0px)" } : { opacity: 0, filter: "blur(4px)" }}
                transition={SPRING}
              >
                {held.length ? held.map((k) => <Kbd key={k}>{k}</Kbd>) : "press new keys"}
                <span aria-hidden className="caret-blink ml-0.5 h-3.5 w-px bg-shu" />
              </motion.span>
            </span>
          </button>
        );
      })}
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* The section                                                         */
/* ------------------------------------------------------------------ */

export function Features() {
  return (
    <MotionConfig reducedMotion="user">
      {/* The caret's blink, and two focus rules strong enough to win over
          the page-wide :focus-visible one: the demo search boxes show focus
          with their caret like the app does, and the keyboard-driven demos
          draw their ring inside, where the rounded demo area can't clip it.
          On touch, the 28px chips and buttons reach 44px with an invisible
          margin of hit area, and the chip rows make room above for it. */}
      <style>{`@keyframes caret-blink{0%,45%{opacity:1}55%,100%{opacity:0}}.caret-blink{animation:caret-blink 1s steps(1,end) infinite}@media (prefers-reduced-motion: reduce){.caret-blink{animation:none}}.feat-input:focus-visible{outline:none}.feat-group:focus-visible{outline:2px solid var(--shu);outline-offset:-2px;border-radius:16px 16px 0 0}@media (pointer:coarse){#features button.press.h-7{position:relative}#features button.press.h-7::after{content:"";position:absolute;inset:-8px -2px}#features [data-scroll-x]{padding-top:8px;margin-top:-8px}}`}</style>
      <section
        id="features"
        aria-labelledby="features-title"
        className="w-full"
      >
        <div className="max-w-2xl">
          <h2 id="features-title" className="font-display text-[2.25rem] leading-[1.08] text-ink sm:text-5xl">
            What it does
          </h2>
          <p className="mt-4 text-lg leading-relaxed text-dim">
            Everything below is live and works the way it does in the app, with
            the same keys.
          </p>
        </div>

        <div className="mt-16 flex flex-col gap-20 sm:mt-20 sm:gap-28">
          <Showcase
            print={suddenShower}
            printAlt="Sudden Shower over Shin-Ōhashi Bridge and Atake, a woodblock print by Hiroshige"
            crop={{ scale: 1, origin: "50% 50%" }}
            credit={
              <>
                <i>Sudden Shower over Shin-Ōhashi Bridge and Atake</i>, Hiroshige, 1857
              </>
            }
            title="Finds the words it misread"
            body={
              <>
                OCR sometimes reads an I as an l, or an O as a 0. Those screenshots still turn up,
                after every exact match, marked as a near match.
              </>
            }
            hint={
              <>
                Try <Code>inv0ice</Code>
              </>
            }
            height={300}
          >
            <NearDemo />
          </Showcase>

          <Showcase
            flip
            print={redFuji}
            printAlt="Fine Wind, Clear Morning (Red Fuji), a woodblock print by Hokusai"
            crop={{ scale: 1.35, origin: "80% 35%" }}
            credit={
              <>
                <i>Fine Wind, Clear Morning</i>, Hokusai, c. 1831
              </>
            }
            title="Bursts fold into one"
            body="Six screenshots of the same chat, taken while you scrolled, show as one tile. The rest are one key away."
            hint={
              <>
                <Kbd>ctrl</Kbd> <Kbd>e</Kbd> unfolds them
              </>
            }
            height={340}
          >
            <BurstDemo />
          </Showcase>

          <Showcase
            print={redFuji}
            printAlt="The sky of Fine Wind, Clear Morning (Red Fuji), a woodblock print by Hokusai"
            crop={{ scale: 2.3, origin: "8% 8%" }}
            credit={
              <>
                <i>Fine Wind, Clear Morning</i>, Hokusai, c. 1831
              </>
            }
            title="Copy just the lines you need"
            body="Drag a box over an open screenshot to copy the lines inside it, or click one line to copy only that."
            hint="Drag across the booking"
            height={300}
          >
            <CopyDemo />
          </Showcase>
        </div>

        <ul className="mt-24 divide-y divide-line sm:mt-32">
          <Card
            height={420}
            title="Filters for where and when"
            body={
              <>
                <Code>in:discord</Code>, <Code>date:yesterday</Code>, <Code>before:aug</Code>. They
                mix with words, and dim once they&apos;re complete.
              </>
            }
          >
            <FiltersDemo />
          </Card>
          <Card
            height={420}
            title="Clear out a hundred at once"
            body={
              <>
                Search <Code>otp</Code>, mark every result, move them to the trash. Nothing is
                deleted, and Ctrl+Z puts them all back.
              </>
            }
          >
            <TrashDemo />
          </Card>
          <Card
            title="Copied images count too"
            body={
              <>
                Turn it on and anything you copy, even if it was never saved, lands in{" "}
                <Code>Pictures/Clipboard</Code> and becomes searchable. Off by default.
              </>
            }
          >
            <ClipboardDemo />
          </Card>
          <Card
            title="Hindi, Marathi, Nepali, and vertical text"
            body="Turn on Devanagari in settings and it reads that too. Vertical Japanese and sideways chart labels read as well."
          >
            <ScriptsDemo />
          </Card>
          <Card
            wide
            title="Every shortcut is yours"
            body="Pick a command and press the new keys. Escape cancels, Delete puts the default back, and keys already in use are refused."
          >
            <ShortcutsDemo />
          </Card>
        </ul>
      </section>
    </MotionConfig>
  );
}
