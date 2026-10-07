import { Row } from "./Frame";
import { GithubIcon, HeartIcon } from "./Header";
import { Mark } from "./Mark";
import { StarCount } from "./GithubStars";
import { BUTTONDOWN, REPO, REPO_SLUG, SAVED_STARS, SPONSOR } from "@/lib/links";

const INSET = "px-5 sm:px-8 lg:px-12";

export function SiteFooter({ stars = SAVED_STARS }: { stars?: number }) {
  return (
    <Row as="footer" divider={false} inner={`${INSET} pt-12 pb-16`}>
      <Updates />
      <div className="mt-12 flex flex-col gap-10 border-t border-line pt-12 text-sm text-dim sm:flex-row sm:justify-between">
        <div className="max-w-sm">
          <a href="/" className="flex items-center gap-2.5 font-medium text-ink">
            <Mark size={22} className="rounded-[6px]" />
            gyotaku
          </a>
          <p className="mt-3 leading-relaxed">
            Gyotaku (魚拓) is the Japanese way of keeping a catch: the fish is
            inked and pressed onto paper. This keeps your screenshots&apos;
            words the same way.
          </p>
          <p className="mt-3">
            GPL-3.0 · made by{" "}
            <a href="https://github.com/xevrion" className="text-ink underline decoration-line-strong underline-offset-4 transition-[text-decoration-color] duration-150 hover:decoration-current">
              xevrion
            </a>
          </p>
          <p className="mt-5 leading-relaxed">
            gyotaku is free and stays free. If it saves you time, sponsoring
            keeps it maintained.
          </p>
          <a
            href={SPONSOR}
            className="press mt-3 inline-flex h-9 items-center gap-2 rounded-[10px] border border-line px-3.5 text-[14px] font-medium text-ink hover:border-line-strong"
          >
            <HeartIcon className="size-3.5" />
            Sponsor gyotaku
          </a>
        </div>
        <div className="grid grid-cols-2 gap-x-10 gap-y-8 self-start sm:grid-cols-[auto_auto]">
          <nav aria-label="Systems" className="flex flex-col gap-2">
            <span className="text-faint">Systems</span>
            <FooterLink href="/mac">macOS</FooterLink>
            <FooterLink href="/windows">Windows</FooterLink>
            <FooterLink href="/linux">Linux</FooterLink>
          </nav>
          <nav aria-label="Project" className="flex flex-col gap-2">
            <span className="text-faint">Project</span>
            <FooterLink href={REPO}>
              <span className="inline-flex items-center gap-1.5">
                <GithubIcon className="size-3.5" />
                <StarCount repo={REPO_SLUG} saved={stars} />
              </span>
            </FooterLink>
            <FooterLink href={`${REPO}/releases`}>Releases</FooterLink>
            <FooterLink href={`${REPO}/blob/main/docs/usage.md`}>Docs</FooterLink>
            <FooterLink href={`${REPO}#roadmap`}>Roadmap</FooterLink>
          </nav>
        </div>
      </div>
    </Row>
  );
}

// The email signup. A plain form that posts straight to Buttondown, with no
// script of theirs on the page. Until the newsletter exists, the same spot
// points at watching releases on GitHub, so links to #updates (from the app
// and the README) always land on something that works.
function Updates() {
  return (
    <section
      id="updates"
      aria-labelledby="updates-title"
      className="flex scroll-mt-20 flex-col gap-5 sm:flex-row sm:items-center sm:justify-between"
    >
      <div>
        <h2 id="updates-title" className="text-[17px] font-medium text-ink">
          Get updates
        </h2>
        <p className="mt-1 text-sm text-dim">New releases and features, no spam.</p>
      </div>
      {BUTTONDOWN ? (
        <form
          action={`https://buttondown.com/api/emails/embed-subscribe/${BUTTONDOWN}`}
          method="post"
          target="_blank"
          className="flex w-full gap-2 sm:w-auto"
        >
          <label htmlFor="updates-email" className="sr-only">
            Email address
          </label>
          <input
            id="updates-email"
            type="email"
            name="email"
            required
            autoComplete="email"
            placeholder="you@example.com"
            className="h-10 min-w-0 flex-1 rounded-[10px] border border-line bg-panel px-3 text-base text-ink outline-hidden placeholder:text-faint focus-visible:border-line-strong sm:w-64 sm:flex-none sm:text-sm"
          />
          <input type="hidden" name="embed" value="1" />
          <button
            type="submit"
            className="press h-10 shrink-0 rounded-[10px] bg-ink px-4 text-sm font-medium text-bg"
          >
            Subscribe
          </button>
        </form>
      ) : (
        <a
          href={REPO}
          className="press inline-flex h-10 items-center gap-2 self-start rounded-[10px] border border-line px-4 text-sm font-medium text-ink hover:border-line-strong sm:self-auto"
        >
          <GithubIcon className="size-3.5" />
          Watch releases on GitHub
        </a>
      )}
    </section>
  );
}

function FooterLink({ href, children }: { href: string; children: React.ReactNode }) {
  return (
    <a href={href} className="py-0.5 transition-colors duration-150 hover:text-ink">
      {children}
    </a>
  );
}
