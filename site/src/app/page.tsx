import Image from "next/image";
import { FAQ, Faq } from "@/components/Faq";
import { JsonLd, SITE_GRAPH, faqPage } from "@/components/JsonLd";
import { Row } from "@/components/Frame";
import { Features } from "@/components/Features";
import { Header } from "@/components/Header";
import { HeroInstall } from "@/components/HeroInstall";
import InstallTabs from "@/components/InstallTabs";
import Numbers from "@/components/Numbers";
import SearchDemo from "@/components/SearchDemo";
import { SiteFooter } from "@/components/SiteFooter";
import { REPO, REPO_SLUG, SAVED_STARS } from "@/lib/links";
import greatWave from "@/assets/prints/great-wave.jpg";

// The page is static, rebuilt at most once an hour so the star count it
// ships with stays close; the browser then asks GitHub for the live one.
export const revalidate = 3600;

async function stars(): Promise<number> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO_SLUG}`, {
      headers: { Accept: "application/vnd.github+json" },
      next: { revalidate: 3600 },
    });
    const data = (await res.json()) as { stargazers_count?: unknown };
    return typeof data.stargazers_count === "number" ? data.stargazers_count : SAVED_STARS;
  } catch {
    return SAVED_STARS;
  }
}

const VERSION = "0.1.6";

// Everything gyotaku ever does over the network. Kept honest with the
// README's Privacy section.
const traffic = [
  { when: "setup", what: "text reading model", result: "once, checksum pinned" },
  { when: "setup", what: "ONNX Runtime", result: "once, checksum pinned" },
  { when: "after", what: "your screenshots", result: "never leave" },
  { when: "after", what: "usage data", result: "never collected" },
];

function Title({ id, children }: { id: string; children: React.ReactNode }) {
  return (
    <h2 id={id} className="font-display text-[2.25rem] leading-[1.08] text-ink sm:text-5xl">
      {children}
    </h2>
  );
}

// Every section's content sits the same distance inside the frame's rails,
// so left edges line up all the way down the page.
const INSET = "px-5 sm:px-8 lg:px-12";
// Room above and below a section's content, between two dividers.
const SPACE = "py-20 sm:py-28";

export default async function Home() {
  const saved = await stars();

  return (
    <>
      <Header stars={saved} />

      <main id="top" className="overflow-x-clip">
        {/* Hero */}
        <Row as="section" inner={`flex flex-col items-center text-center ${INSET} pt-16 pb-16 sm:pt-24 sm:pb-20`}>
          <a
            href={`${REPO}/releases/latest`}
            className="press group inline-flex h-8 items-center gap-2 rounded-full border border-line bg-panel pr-3 pl-1 text-[13px] text-dim hover:text-ink"
          >
            <span className="rounded-full bg-sunk px-2 py-0.5 font-medium text-ink tabular-nums">v{VERSION}</span>
            Free and open source
            <svg viewBox="0 0 16 16" aria-hidden className="size-3 transition-[translate] duration-200 ease-out group-hover:translate-x-0.5" fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round">
              <path d="M3.5 8h9M9 4.5 12.5 8 9 11.5" />
            </svg>
          </a>

          <h1 className="font-display mt-7 text-[2.9rem] leading-[1.02] text-ink min-[400px]:text-[3.3rem] sm:text-[4.5rem] sm:leading-[1]">
            Ctrl F for your <span className="found">screenshots</span>
          </h1>

          <p className="mt-5 max-w-[34rem] text-[17px] leading-relaxed text-dim sm:text-lg">
            gyotaku reads the text in every screenshot on your computer, so you
            can search your screenshots by any word you remember. Free, open
            source and fully offline.
          </p>

          <div className="mt-8">
            <HeroInstall />
          </div>

          <p className="mt-4 text-[13px] text-faint">macOS, Windows and Linux</p>
        </Row>

        {/* The product, live, on Hokusai's Great Wave: a woodblock print, the
            same printmaking tradition gyotaku is named after. */}
        <Row as="section" aria-label="Try the search" inner="px-3 pt-3 pb-4 sm:px-5 sm:pt-5 sm:pb-5">
          <div className="relative overflow-hidden rounded-[18px] sm:rounded-[22px]">
            <Image
              src={greatWave}
              alt="The Great Wave off Kanagawa, a woodblock print by Hokusai"
              fill
              loading="eager"
              fetchPriority="high"
              placeholder="blur"
              sizes="(min-width: 1152px) 1104px, 100vw"
              className="object-cover object-[30%_40%]"
            />
            <div className="relative px-3 pt-10 pb-6 sm:px-14 sm:pt-16 sm:pb-12">
              <SearchDemo onWallpaper />
            </div>
          </div>
          <p className="mt-3 text-right text-[12px] text-faint">
            Background: <i>The Great Wave off Kanagawa</i>, Hokusai, c. 1831
          </p>
        </Row>

        <Row inner={`${INSET} ${SPACE}`}>
          <Features />
        </Row>

        <Row inner={`${INSET} ${SPACE}`}>
          <Numbers />
        </Row>

        {/* Install */}
        <Row as="section" id="install" aria-labelledby="install-title" className="band" inner={`${INSET} ${SPACE}`}>
          <div>
            <Title id="install-title">Install in one line</Title>
            <p className="mt-3 max-w-xl text-lg leading-relaxed text-dim">
              No admin rights and no developer tools. Run it again any time to
              update.
            </p>
            <div className="mt-10 max-w-3xl">
              <InstallTabs />
            </div>
          </div>
        </Row>

        {/* Privacy */}
        <Row as="section" aria-labelledby="private" inner={`${INSET} ${SPACE}`}>
          <div className="grid items-start gap-10 md:grid-cols-[1fr_1.05fr] md:gap-14">
            <div>
              <Title id="private">Nothing leaves your computer</Title>
              <p className="mt-4 text-lg leading-relaxed text-dim">
                Screenshots hold passwords, codes, chats and bank details.
                gyotaku reads them on your own CPU and keeps the index on your
                own disk. No account, no telemetry, no update checks.
              </p>
            </div>
            <figure className="overflow-hidden rounded-2xl bg-panel shadow-[var(--shadow)]">
              <figcaption className="flex items-center justify-between border-b border-line px-4 py-3 text-[13px] text-dim">
                <span>Everything gyotaku does online</span>
                <span className="font-mono text-[12px] text-faint">all time</span>
              </figcaption>
              <table className="w-full font-mono text-[12px] sm:text-[13px]">
                <thead className="sr-only">
                  <tr>
                    <th scope="col">When</th>
                    <th scope="col">What</th>
                    <th scope="col">How often</th>
                  </tr>
                </thead>
                <tbody>
                  {traffic.map((t, i) => (
                    <tr key={t.what} className={i ? "border-t border-line" : ""}>
                      <td className="py-3 pr-2 pl-4 align-top text-faint">{t.when}</td>
                      <td className="py-3 pr-2 align-top text-ink">{t.what}</td>
                      <td className={`py-3 pr-4 text-right align-top ${t.when === "after" ? "font-medium text-ink" : "text-dim"}`}>
                        {t.result}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </figure>
          </div>
        </Row>

        {/* FAQ */}
        <Row as="section" id="faq" aria-labelledby="faq-title" inner={`${INSET} ${SPACE}`}>
          <div className="grid gap-10 md:grid-cols-[0.8fr_1.2fr] md:gap-14">
            <div>
              <Title id="faq-title">Questions</Title>
              <p className="mt-4 text-dim">
                Something else?{" "}
                <a href={`${REPO}/issues`} className="text-ink underline decoration-line-strong underline-offset-4 transition-[text-decoration-color] duration-150 hover:decoration-current">
                  Open an issue
                </a>
                .
              </p>
            </div>
            <Faq />
          </div>
        </Row>
      </main>

      <SiteFooter stars={saved} />
      <JsonLd graph={[...SITE_GRAPH, faqPage(FAQ)]} />
    </>
  );
}
