"use client";

import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Mark } from "./Mark";
import { StarCount } from "./GithubStars";
import { ThemeToggle } from "./ThemeToggle";
import { Row } from "./Frame";
import { REPO, REPO_SLUG, SAVED_STARS, SPONSOR } from "@/lib/links";
import { useReducedMotion } from "@/lib/use-reduced-motion";

// The top edge of the page's frame: the same rails and corner squares as
// every section, so the bar reads as part of the drawing, not something
// floating over it. Opaque, so nothing shows through while scrolling.

const LINKS = [
  { href: "#features", id: "features", label: "Features" },
  { href: "#install", id: "install", label: "Install" },
  { href: "#faq", id: "faq", label: "FAQ" },
  { href: `${REPO}/blob/main/docs/usage.md`, id: null, label: "Docs" },
] as const;

const ICON = { type: "spring", duration: 0.3, bounce: 0 } as const;

// Which section is under the middle of the screen. One observer, a thin band
// across the viewport's middle, so only one section can be in it at a time.
function useActiveSection() {
  const [active, setActive] = useState<string | null>(null);
  useEffect(() => {
    const els = LINKS.flatMap((l) => (l.id ? [document.getElementById(l.id)] : [])).filter(
      (el): el is HTMLElement => el !== null,
    );
    const seen = new Map<string, boolean>();
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) seen.set(e.target.id, e.isIntersecting);
        setActive(els.find((el) => seen.get(el.id))?.id ?? null);
      },
      { rootMargin: "-45% 0px -50% 0px" },
    );
    els.forEach((el) => io.observe(el));
    return () => io.disconnect();
  }, []);
  return active;
}

// `base` is "/" on pages other than the home page, so the section links lead
// back to the home page's sections instead of nowhere.
export function Header({ stars = SAVED_STARS, base = "" }: { stars?: number; base?: string }) {
  const active = useActiveSection();
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const sheet = useRef<HTMLDivElement>(null);
  const reduce = useReducedMotion();

  // Escape closes the menu and gives focus back to the button; growing past
  // the phone layout closes it too, so it never lingers off-screen.
  useEffect(() => {
    if (!open) return;
    sheet.current?.querySelector<HTMLElement>("a")?.focus({ preventScroll: true });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        button.current?.focus();
      }
    };
    const wide = matchMedia("(min-width: 768px)");
    const onWide = () => wide.matches && setOpen(false);
    window.addEventListener("keydown", onKey);
    wide.addEventListener("change", onWide);
    return () => {
      window.removeEventListener("keydown", onKey);
      wide.removeEventListener("change", onWide);
    };
  }, [open]);

  const swap = reduce
    ? { initial: false as const, animate: { opacity: 1 }, exit: { opacity: 0 } }
    : {
        initial: { opacity: 0, scale: 0.25, filter: "blur(4px)" },
        animate: { opacity: 1, scale: 1, filter: "blur(0px)" },
        exit: { opacity: 0, scale: 0.25, filter: "blur(4px)" },
      };

  return (
    <Row as="header" className="sticky top-0 z-40 bg-bg">
      <div className="grid h-16 grid-cols-[1fr_auto] items-center px-4 sm:px-6 md:grid-cols-[1fr_auto_1fr]">
        <a
          href={base ? "/" : "#top"}
          onClick={() => setOpen(false)}
          className="press -mx-1.5 flex items-center gap-2.5 justify-self-start rounded-lg px-1.5 py-1 text-[15px] font-semibold tracking-[-0.01em] text-ink"
        >
          <Mark size={24} className="rounded-[6px]" />
          gyotaku
        </a>

        <nav aria-label="Sections" className="hidden items-center gap-1 text-[14px] md:flex">
          {LINKS.map((l) => (
            <a
              key={l.label}
              href={l.href.startsWith("#") ? base + l.href : l.href}
              aria-current={l.id && active === l.id ? "true" : undefined}
              className="rounded-lg px-3 py-1.5 text-dim transition-colors duration-150 hover:text-ink aria-[current=true]:text-ink"
            >
              {l.label}
            </a>
          ))}
        </nav>

        <div className="flex items-center gap-1 justify-self-end">
          <a
            href={SPONSOR}
            className="press group hidden h-9 items-center gap-1.5 rounded-lg px-2.5 text-[14px] text-dim hover:text-ink md:flex"
          >
            <HeartIcon className="size-3.5 transition-[color,fill] duration-150 group-hover:fill-shu group-hover:text-shu" />
            Sponsor
          </a>
          <a
            href={REPO}
            className="press hidden h-9 items-center gap-1.5 rounded-lg px-2.5 text-[14px] text-dim hover:text-ink sm:flex"
          >
            <GithubIcon />
            <span className="sr-only">gyotaku on GitHub, stars:</span>
            <StarCount repo={REPO_SLUG} saved={stars} />
          </a>
          <ThemeToggle />
          <a
            href={`${base}#install`}
            className="press ml-1.5 hidden h-9 items-center rounded-[10px] bg-ink px-4 text-[14px] font-medium text-bg sm:flex"
          >
            Install
          </a>
          <button
            ref={button}
            type="button"
            aria-expanded={open}
            aria-controls="site-menu"
            aria-label={open ? "Close menu" : "Open menu"}
            onClick={() => setOpen((o) => !o)}
            className="press relative flex size-9 items-center justify-center rounded-lg text-ink md:hidden"
          >
            <AnimatePresence initial={false} mode="popLayout">
              <motion.svg
                key={open ? "close" : "menu"}
                {...swap}
                transition={ICON}
                viewBox="0 0 16 16"
                className="size-4"
                fill="none"
                stroke="currentColor"
                strokeWidth={1.5}
                strokeLinecap="round"
                aria-hidden
              >
                {open ? <path d="M4 4l8 8M12 4l-8 8" /> : <path d="M2.5 5.5h11M2.5 10.5h11" />}
              </motion.svg>
            </AnimatePresence>
          </button>
        </div>
      </div>

      {/* The phone menu: unrolls downward from the header's bottom line,
          inside the same frame. Kept mounted and inert while closed, so it
          can animate both ways and never takes focus when hidden. */}
      <div
        id="site-menu"
        ref={sheet}
        data-open={open}
        inert={!open}
        className="site-menu absolute inset-x-0 top-full border-b border-line bg-bg px-3 sm:px-6 md:hidden"
      >
        <nav aria-label="Menu" className="mx-auto flex max-w-6xl flex-col border-x border-line px-4 pt-2 pb-5">
          {LINKS.map((l) => (
            <a
              key={l.label}
              href={l.href.startsWith("#") ? base + l.href : l.href}
              onClick={() => setOpen(false)}
              className="flex h-12 items-center border-b border-line text-[17px] text-ink transition-colors duration-150 last:border-b-0"
            >
              {l.label}
            </a>
          ))}
          <a
            href={SPONSOR}
            onClick={() => setOpen(false)}
            className="flex h-12 items-center gap-2.5 text-[17px] text-ink"
          >
            <HeartIcon className="size-4 text-shu" />
            Sponsor gyotaku
          </a>
          <div className="mt-4 flex items-center gap-2">
            <a
              href={`${base}#install`}
              onClick={() => setOpen(false)}
              className="press flex h-11 flex-1 items-center justify-center rounded-[12px] bg-ink text-[15px] font-medium text-bg"
            >
              Install gyotaku
            </a>
            <a
              href={REPO}
              className="press flex h-11 items-center gap-2 rounded-[12px] border border-line px-4 text-[15px] text-ink"
            >
              <GithubIcon />
              <StarCount repo={REPO_SLUG} saved={stars} />
            </a>
          </div>
        </nav>
      </div>
    </Row>
  );
}

export function HeartIcon({ className = "size-4" }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" aria-hidden className={className} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinejoin="round">
      <path d="M8 13.5S2 10 2 5.75A2.75 2.75 0 0 1 8 4.5a2.75 2.75 0 0 1 6 1.25C14 10 8 13.5 8 13.5Z" />
    </svg>
  );
}

export function GithubIcon({ className = "size-4" }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden className={className}>
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
    </svg>
  );
}
