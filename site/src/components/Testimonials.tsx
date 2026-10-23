import Image, { type StaticImageData } from "next/image";
import adityawaslost from "@/assets/testimonials/adityawaslost.webp";
import ashutosh from "@/assets/testimonials/Ashutosh_Bind15.webp";
import astriknormal from "@/assets/testimonials/astriknormal.webp";
import brianhadu from "@/assets/testimonials/BrianHaDu.webp";
import cgranier from "@/assets/testimonials/cgranier.webp";
import elrbtclr from "@/assets/testimonials/elrbtclr.webp";
import endrymagolas from "@/assets/testimonials/EndryMagolas.webp";
import hecodesforme from "@/assets/testimonials/hecodesforme.webp";
import itzkashan from "@/assets/testimonials/itzKashan2912.webp";
import mitansh from "@/assets/testimonials/mitansh_j07.webp";
import montpy from "@/assets/testimonials/mont_py.webp";
import mutahir from "@/assets/testimonials/mutahir.webp";
import mystic0x from "@/assets/testimonials/_mystic0x.webp";
import pranvv27 from "@/assets/testimonials/pranvv27.webp";
import rechterroni from "@/assets/testimonials/RechterRoni.webp";
import siddhimahi from "@/assets/testimonials/Siddhimahi.webp";
import techdoncooper from "@/assets/testimonials/techdonCooper.webp";
import thenightshipper from "@/assets/testimonials/thenightshipper.webp";
import zuhayrdev from "@/assets/testimonials/zuhayr_dev.webp";

// Replies to the launch post on X, word for word (trimmed at the end where a
// reply went on to ask something, never edited). Avatars are cropped from
// screenshots of the replies themselves.

const LAUNCH_POST = "https://x.com/xevrion_the1/status/2107047524662157818";

// How many of the smaller quotes a phone shows before "show more".
const PHONE_SHOWS = 8;

type Quote = { text: string; name: string; handle: string; avatar: StaticImageData };

const FEATURED: Quote = {
  text: "the screenshots folder was always a database, just badly indexed",
  name: "Roni Rechter",
  handle: "RechterRoni",
  avatar: rechterroni,
};

// Strongest first: the pain, then privacy and open source, then the craft,
// then the people who just wanted it.
const QUOTES: Quote[] = [
  {
    text: "A ctrl+f for a folder full of screenshots makes immediate sense. Keeping it offline is a relief when half those images contain private stuff.",
    name: "Abhishek kothari",
    handle: "thenightshipper",
    avatar: thenightshipper,
  },
  {
    text: "ctrl+f for screenshots solves a problem literally everyone has.",
    name: "Sujal",
    handle: "hecodesforme",
    avatar: hecodesforme,
  },
  {
    text: "bro could have charged, but went opensource, this is the real AURA!",
    name: "Muhammad Kashan Ashraf",
    handle: "itzKashan2912",
    avatar: itzkashan,
  },
  {
    text: "Okay this is solving such a niche problem that even coming up with the idea is pure genius",
    name: "Aniket",
    handle: "astriknormal",
    avatar: astriknormal,
  },
  {
    text: "Ctrl+F is the missing half of “save for later”",
    name: "Endry Magolas",
    handle: "EndryMagolas",
    avatar: endrymagolas,
  },
  {
    text: "the offline feature is a must-have for privacy, it's a real win for users",
    name: "Brian Hadu",
    handle: "BrianHaDu",
    avatar: brianhadu,
  },
  {
    text: "wow, some real voodoo! instafollow, cherry on top is that it is GPUI",
    name: "Monty",
    handle: "mont_py",
    avatar: montpy,
  },
  {
    text: "I'd use this to find an old error screenshot by the exception name. Handy little tool.",
    name: "Zuhayr Khan",
    handle: "zuhayr_dev",
    avatar: zuhayrdev,
  },
  {
    text: "Absolutely love this and as a person who's super dependant on screenshots for conversation or jira this is very helpful",
    name: "Devanshi Doshi",
    handle: "techdonCooper",
    avatar: techdoncooper,
  },
  {
    text: "Ctrl+f for a screenshots folder, fully offline and open source, hits a real pain.\nLocal search for buried captures is such a practical build",
    name: "Mitansh",
    handle: "mitansh_j07",
    avatar: mitansh,
  },
  {
    text: "I was just trying to find an old screenshot today. Thanks!",
    name: "Carlos Granier",
    handle: "cgranier",
    avatar: cgranier,
  },
  {
    text: "just stalked you repo\ndamn you're a cracked dev",
    name: "Aditya",
    handle: "adityawaslost",
    avatar: adityawaslost,
  },
  {
    text: "gahdamn you beat me to it! wanted to build something like this. great work!",
    name: "am",
    handle: "elrbtclr",
    avatar: elrbtclr,
  },
  {
    text: "thanks for the native linux support",
    name: "Ashutosh",
    handle: "Ashutosh_Bind15",
    avatar: ashutosh,
  },
  {
    text: "Exactly the problem I and many would have\nThanks for this - will try it out",
    name: "mutahir",
    handle: "mutahir",
    avatar: mutahir,
  },
  {
    text: "looks like something that will actually help me",
    name: "Siddhi Maheshwari",
    handle: "Siddhimahi",
    avatar: siddhimahi,
  },
  {
    text: "And the launch video stole the show!\nDef using this",
    name: "mystic0x",
    handle: "_mystic0x",
    avatar: mystic0x,
  },
  {
    text: "thiss goonaa explode!!",
    name: "Pranavvv",
    handle: "pranvv27",
    avatar: pranvv27,
  },
];

export function Testimonials() {
  return (
    <section aria-labelledby="said" className="w-full">
      <h2 id="said" className="font-display text-[2.25rem] leading-[1.08] text-ink sm:text-5xl">
        What people said
      </h2>
      <p className="mt-3 max-w-xl text-lg leading-relaxed text-dim">
        Replies to the{" "}
        <a
          href={LAUNCH_POST}
          rel="noopener"
          className="text-ink underline decoration-line-strong underline-offset-4 transition-[text-decoration-color] duration-150 hover:decoration-current"
        >
          launch post on X
        </a>
        , which got 1,352 likes and 846 bookmarks.
      </p>

      {/* One reply set large: it says what gyotaku is better than we did. */}
      <figure className="mt-12 max-w-3xl sm:mt-16">
        <blockquote className="font-display text-[1.75rem] leading-[1.18] text-ink sm:text-[2.5rem] sm:leading-[1.12]">
          <p className="-indent-[0.42em]">
            <span aria-hidden className="text-faint">
              &ldquo;
            </span>
            {FEATURED.text}
            <span aria-hidden className="text-faint">
              &rdquo;
            </span>
          </p>
        </blockquote>
        <figcaption className="mt-6">
          <Attribution quote={FEATURED} size={36} />
        </figcaption>
      </figure>

      {/* The rest as a column flow of plain quotes, divided by hairlines
          rather than boxed, so it reads like a page of replies. A phone
          shows the first few and keeps the rest a tap away; the toggle is a
          checkbox, so it works without JavaScript. */}
      <div className="group">
        <div className="mt-14 gap-x-12 border-t border-line sm:columns-2 lg:columns-3">
          {QUOTES.map((q, i) => (
            <figure
              key={q.handle}
              className={`break-inside-avoid border-b border-line py-7 ${
                i >= PHONE_SHOWS ? "max-sm:hidden max-sm:group-has-[input:checked]:block" : ""
              }`}
            >
              <blockquote className="text-[16px] leading-relaxed whitespace-pre-line text-ink sm:text-[17px]">
                <p>{q.text}</p>
              </blockquote>
              <figcaption className="mt-4">
                <Attribution quote={q} size={28} />
              </figcaption>
            </figure>
          ))}
        </div>
        <label className="press mt-6 inline-flex h-11 cursor-pointer items-center rounded-[10px] border border-line px-4 text-[15px] font-medium text-ink select-none has-[input:focus-visible]:outline-2 has-[input:focus-visible]:outline-offset-2 has-[input:focus-visible]:outline-shu group-has-[input:checked]:hidden sm:hidden">
          <input type="checkbox" className="sr-only" />
          Show {QUOTES.length - PHONE_SHOWS} more
        </label>
      </div>
    </section>
  );
}

function Attribution({ quote, size }: { quote: Quote; size: number }) {
  return (
    <div className="flex items-center gap-3 text-[14px]">
      <Image
        src={quote.avatar}
        alt={quote.name}
        width={size}
        height={size}
        className="shrink-0 rounded-full outline outline-1 -outline-offset-1 outline-[var(--outline)]"
      />
      <span className="flex min-w-0 flex-wrap items-baseline gap-x-2">
        <cite className="font-medium text-ink not-italic">{quote.name}</cite>
        <a
          href={`https://x.com/${quote.handle}`}
          rel="noopener"
          aria-label={`${quote.name} on X, @${quote.handle}`}
          className="text-dim transition-colors duration-150 hover:text-ink"
        >
          @{quote.handle}
        </a>
      </span>
    </div>
  );
}
