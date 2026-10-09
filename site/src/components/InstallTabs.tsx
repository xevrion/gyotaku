"use client";

import { AnimatePresence, motion, MotionConfig } from "motion/react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";

export type Os = "linux" | "windows" | "macos";

export type Tab = {
  id: Os;
  label: string;
  prompt: string;
  command: string;
  note: string;
  update: string;
  /** The page with everything about running gyotaku on this system. */
  page: string;
};

export const TABS: Tab[] = [
  {
    id: "linux",
    label: "Linux",
    prompt: "$",
    command:
      "curl -fsSL https://raw.githubusercontent.com/xevrion/gyotaku/main/install.sh | sh",
    note: "x86_64 and arm64, on any distro with glibc 2.35 or newer.",
    update:
      "Run it again any time to update. The old window closes and the reader restarts on the new version.",
    page: "/linux",
  },
  {
    id: "windows",
    label: "Windows",
    prompt: "PS>",
    command:
      "irm https://raw.githubusercontent.com/xevrion/gyotaku/main/install.ps1 | iex",
    note: "In PowerShell, on Windows 10 and 11. No admin needed.",
    update:
      "Run it again any time to update. It closes the running copy and opens the new one.",
    page: "/windows",
  },
  {
    id: "macos",
    label: "macOS",
    prompt: "$",
    command:
      "curl -fsSL https://raw.githubusercontent.com/xevrion/gyotaku/main/install.sh | sh",
    note: "In Terminal, on Apple Silicon Macs. No admin needed. Alt Shift S opens the search window.",
    update:
      "Run it again any time to update. The old window closes and the reader restarts on the new version.",
    page: "/mac",
  },
];

const spring = { type: "spring", duration: 0.3, bounce: 0 } as const;

export function detect(): Os {
  const ua = navigator.userAgent;
  if (/Windows/i.test(ua)) return "windows";
  if (/Mac OS X|Macintosh/i.test(ua) && !/iPhone|iPad/i.test(ua)) return "macos";
  return "linux";
}

// Phones can't run it. Android says "Linux" in its user agent, so this has
// to be asked before trusting detect().
export function onPhone(): boolean {
  const data = (navigator as Navigator & { userAgentData?: { mobile?: boolean } }).userAgentData;
  return data?.mobile ?? /Android|iPhone|iPad|iPod|Mobile/i.test(navigator.userAgent);
}

export default function InstallTabs() {
  const [os, setOs] = useState<Os>("linux");
  // Only a click animates. Settling on the visitor's OS after the page loads
  // happens before anyone is looking, so it just snaps.
  const [animate, setAnimate] = useState(false);
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    setOs(detect());
  }, []);

  const pick = (next: Os) => {
    setAnimate(true);
    setOs(next);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    const edge = e.key === "Home" ? 0 : e.key === "End" ? TABS.length - 1 : -1;
    if (!step && edge < 0) return;
    e.preventDefault();
    const at = TABS.findIndex((t) => t.id === os);
    const next = edge >= 0 ? edge : (at + step + TABS.length) % TABS.length;
    pick(TABS[next].id);
    tabs.current[next]?.focus();
  };

  const tab = TABS.find((t) => t.id === os)!;

  return (
    <MotionConfig
      transition={animate ? spring : { duration: 0 }}
      reducedMotion="user"
    >
      <div className="w-full max-w-2xl">
        <div
          role="tablist"
          aria-label="Operating system"
          onKeyDown={onKeyDown}
          className="inline-flex gap-0.5 rounded-[12px] bg-sunk p-1"
        >
          {TABS.map((t, i) => {
            const selected = t.id === os;
            return (
              <button
                key={t.id}
                ref={(el) => {
                  tabs.current[i] = el;
                }}
                role="tab"
                id={`install-tab-${t.id}`}
                aria-selected={selected}
                aria-controls="install-panel"
                tabIndex={selected ? 0 : -1}
                onClick={() => pick(t.id)}
                // Tabs sit edge to edge, so on touch screens the tap area
                // grows up and down only, never into the next tab.
                className={`press relative h-8 rounded-[8px] px-3.5 text-sm font-medium transition-[color] duration-150 pointer-coarse:after:absolute pointer-coarse:after:inset-x-0 pointer-coarse:after:-inset-y-1.5 pointer-coarse:after:content-[''] ${
                  selected ? "text-ink" : "text-dim hover:text-ink"
                }`}
              >
                {selected && (
                  <motion.span
                    layoutId="install-pill"
                    className="absolute inset-0 rounded-[8px] bg-panel shadow-[0_0_0_1px_var(--line),0_1px_2px_rgb(0_0_0/0.06)]"
                  />
                )}
                <span className="relative">{t.label}</span>
              </button>
            );
          })}
        </div>

        <div
          role="tabpanel"
          id="install-panel"
          aria-labelledby={`install-tab-${os}`}
          className="relative mt-3"
        >
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.div
              key={os}
              initial={{ opacity: 0, filter: "blur(4px)", transform: "translateY(4px)" }}
              animate={{ opacity: 1, filter: "blur(0px)", transform: "translateY(0px)" }}
              exit={{ opacity: 0, filter: "blur(4px)", transform: "translateY(-4px)" }}
            >
              <div className="flex items-center gap-1 rounded-[14px] bg-sunk p-1.5 pl-4">
                <div className="min-w-0 flex-1 overflow-x-auto py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                  <code className="whitespace-nowrap font-mono text-[13px] leading-none text-ink">
                    <span className="mr-2 select-none text-faint">{tab.prompt}</span>
                    {tab.command}
                  </code>
                </div>
                <CopyButton text={tab.command} />
              </div>
              <p className="mt-3 text-sm text-dim">{tab.note}</p>
              <p className="mt-1 text-sm text-dim">{tab.update}</p>
              <a
                href={tab.page}
                className="group mt-4 inline-flex items-center gap-1.5 text-sm text-ink underline decoration-line-strong underline-offset-4 transition-[text-decoration-color] duration-150 hover:decoration-current"
              >
                More about gyotaku on {tab.label}
                <svg viewBox="0 0 16 16" aria-hidden className="size-3 transition-[translate] duration-200 ease-out group-hover:translate-x-0.5" fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round">
                  <path d="M3.5 8h9M9 4.5 12.5 8 9 11.5" />
                </svg>
              </a>
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </MotionConfig>
  );
}

// The command for the visitor's own system, on its own: the last thing on
// the page, for someone who's already convinced. Linux on the server, then
// the detected system without any animation.
export function QuickCommand() {
  const [os, setOs] = useState<Os>("linux");
  useEffect(() => {
    setOs(detect());
  }, []);
  const tab = TABS.find((t) => t.id === os)!;

  return (
    <div className="flex w-full max-w-xl items-center gap-1 rounded-[14px] bg-sunk p-1.5 pl-4">
      <div className="min-w-0 flex-1 overflow-x-auto py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        <code className="whitespace-nowrap font-mono text-[13px] leading-none text-ink">
          <span className="mr-2 select-none text-faint">{tab.prompt}</span>
          {tab.command}
        </code>
      </div>
      <CopyButton text={tab.command} primary />
    </div>
  );
}

// One command in a copyable box, for pages that show a single system.
export function Command({ prompt, command }: { prompt: string; command: string }) {
  return (
    <div className="flex w-full items-center gap-1 rounded-[14px] bg-sunk p-1.5 pl-4">
      <div className="min-w-0 flex-1 overflow-x-auto py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        <code className="whitespace-nowrap font-mono text-[13px] leading-none text-ink">
          <span className="mr-2 select-none text-faint">{prompt}</span>
          {command}
        </code>
      </div>
      <CopyButton text={command} />
    </div>
  );
}

function CopyButton({ text, primary = false }: { text: string; primary?: boolean }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      return;
    }
    setCopied(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), 1600);
  };

  const icon = (
    <AnimatePresence mode="popLayout" initial={false}>
      <motion.span
        key={copied ? "check" : "copy"}
        className="grid place-items-center"
        initial={{ opacity: 0, scale: 0.25, filter: "blur(4px)" }}
        animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
        exit={{ opacity: 0, scale: 0.25, filter: "blur(4px)" }}
        transition={spring}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </motion.span>
    </AnimatePresence>
  );

  return (
    <>
      {primary ? (
        // The closing call to action: copying is the whole point, so it's a
        // real button with a word on it. Both words share one cell so the
        // button keeps its width when "Copy" becomes "Copied".
        <button
          type="button"
          onClick={copy}
          className="press flex h-9 shrink-0 items-center gap-1.5 rounded-[8px] bg-shu pr-3 pl-2.5 text-[14px] font-medium text-[var(--on-shu)] shadow-[0_1px_0_rgb(255_255_255/0.2)_inset]"
        >
          {icon}
          <span className="grid">
            <span
              className="col-start-1 row-start-1 transition-[opacity,filter] duration-200 ease-[var(--ease-out)]"
              style={{ opacity: copied ? 0 : 1, filter: copied ? "blur(4px)" : "blur(0px)" }}
            >
              Copy
            </span>
            <span
              aria-hidden
              className="col-start-1 row-start-1 transition-[opacity,filter] duration-200 ease-[var(--ease-out)]"
              style={{ opacity: copied ? 1 : 0, filter: copied ? "blur(0px)" : "blur(4px)" }}
            >
              Copied
            </span>
          </span>
        </button>
      ) : (
        <button
          type="button"
          onClick={copy}
          aria-label="copy command"
          className={`press relative grid size-8 shrink-0 place-items-center rounded-[8px] transition-[color,background-color] duration-150 hover:bg-panel pointer-coarse:after:absolute pointer-coarse:after:-inset-1.5 pointer-coarse:after:content-[''] ${
            copied ? "text-shu" : "text-dim hover:text-ink"
          }`}
        >
          {icon}
        </button>
      )}
      <span aria-live="polite" className="sr-only">
        {copied ? "copied" : ""}
      </span>
    </>
  );
}

function CopyIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <rect x="5.25" y="5.25" width="8" height="8" rx="1.75" />
      <path d="M10.75 5.25v-1.5a1.5 1.5 0 0 0-1.5-1.5h-5a1.5 1.5 0 0 0-1.5 1.5v5a1.5 1.5 0 0 0 1.5 1.5h1.5" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M3.25 8.5l3 3 6.5-7" />
    </svg>
  );
}
