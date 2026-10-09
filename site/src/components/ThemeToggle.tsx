"use client";

// From ui lab (lab.xevrion.dev): the new theme spreads out from the button
// in a circle, through a view transition.

import { useSyncExternalStore } from "react";
import { motion, type Transition } from "motion/react";

type Theme = "light" | "dark";

const DARK_QUERY = "(prefers-color-scheme: dark)";
const ICON_TRANSITION: Transition = { type: "spring", duration: 0.3, bounce: 0 };

// Runs in <head> before first paint, so a saved theme never flashes the other one.
export const themeScript = `try{const t=localStorage.getItem("theme");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t}catch{}`;

function subscribe(onChange: () => void) {
  const media = matchMedia(DARK_QUERY);
  const observer = new MutationObserver(onChange);
  media.addEventListener("change", onChange);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });
  return () => {
    media.removeEventListener("change", onChange);
    observer.disconnect();
  };
}

function getTheme(): Theme {
  const stored = document.documentElement.dataset.theme;
  if (stored === "light" || stored === "dark") return stored;
  return matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

function applyTheme(theme: Theme) {
  // Without this every color transition on the page fires at once and the
  // switch smears instead of snapping.
  const pause = document.createElement("style");
  pause.textContent = "*,*::before,*::after{transition:none!important}";
  document.head.append(pause);

  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem("theme", theme);
  } catch {}

  void document.body.offsetHeight;
  requestAnimationFrame(() => pause.remove());
}

// How soft the wave's edge is: the new theme fades in over this band.
const FEATHER = 80;

// The new theme grows out of the button as a circle with a feathered edge,
// so it reads as a soft wave sweeping the page rather than a hard cut-out.
// The mask and its --reveal-* properties live in globals.css; this only
// positions and drives them. (The same reveal as xevrion's site.)
function setTheme(theme: Theme, x: number, y: number) {
  if (
    !document.startViewTransition ||
    matchMedia("(prefers-reduced-motion: reduce)").matches
  ) {
    applyTheme(theme);
    return;
  }

  const root = document.documentElement;
  // Past the farthest corner by the feather, so the soft band clears the
  // screen and the last corner lands fully on the new theme.
  const radius =
    Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y)) + FEATHER;
  root.style.setProperty("--reveal-x", `${x}px`);
  root.style.setProperty("--reveal-y", `${y}px`);
  root.style.setProperty("--reveal-feather", `${FEATHER}px`);
  // Scoped to this class so no other view transition inherits the mask.
  root.classList.add("theme-reveal");

  const t = document.startViewTransition(() => applyTheme(theme));
  t.ready
    .then(() =>
      root.animate(
        { "--reveal-r": ["0px", `${radius}px`] },
        // The iOS drawer curve over 900ms: the wave visibly sweeps the page.
        // A once-in-a-while action, so it can take its time. Held at the end,
        // or the radius drops back to 0 on the last frame and flashes the
        // old theme before the transition tears down.
        {
          duration: 900,
          easing: "cubic-bezier(0.32, 0.72, 0, 1)",
          fill: "forwards",
          pseudoElement: "::view-transition-new(root)",
        },
      ),
    )
    .catch(() => {});
  t.finished.finally(() => root.classList.remove("theme-reveal"));
}

export function ThemeToggle() {
  const theme = useSyncExternalStore(subscribe, getTheme, () => null);
  const next = theme === "dark" ? "light" : "dark";

  return (
    <button
      type="button"
      aria-label={`Switch to ${next} theme`}
      onClick={(e) => {
        const rect = e.currentTarget.getBoundingClientRect();
        setTheme(next, rect.x + rect.width / 2, rect.y + rect.height / 2);
      }}
      // 36px to look at, 44px to tap on a touch screen (the invisible ring).
      className="relative flex size-9 items-center justify-center rounded-full text-dim transition-[scale,color,background-color] duration-150 ease-out hover:bg-sunk hover:text-ink active:scale-[0.96] motion-reduce:transition-none pointer-coarse:after:absolute pointer-coarse:after:-inset-1 pointer-coarse:after:content-['']"
    >
      {/* The server can't know the theme, so the icons mount once it's known,
          already in place, instead of animating in on every page load. */}
      {theme && (
        <>
          <Icon visible={theme === "light"}>
            <circle cx="8" cy="8" r="2.75" />
            <path d="M8 1.75v1M8 13.25v1M1.75 8h1M13.25 8h1M3.58 3.58l.7.7M11.72 11.72l.7.7M3.58 12.42l.7-.7M11.72 4.28l.7-.7" />
          </Icon>
          <Icon visible={theme === "dark"}>
            <path d="M13.5 9.6A5.75 5.75 0 0 1 6.4 2.5a5.75 5.75 0 1 0 7.1 7.1Z" />
          </Icon>
        </>
      )}
    </button>
  );
}

function Icon({
  visible,
  children,
}: {
  visible: boolean;
  children: React.ReactNode;
}) {
  return (
    <motion.svg
      viewBox="0 0 16 16"
      className="absolute size-4"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      initial={false}
      animate={
        visible
          ? { scale: 1, opacity: 1, filter: "blur(0px)" }
          : { scale: 0.25, opacity: 0, filter: "blur(4px)" }
      }
      transition={ICON_TRANSITION}
    >
      {children}
    </motion.svg>
  );
}
