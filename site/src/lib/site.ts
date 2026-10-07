// Where the site lives, for canonical links, the sitemap and social cards.
// NEXT_PUBLIC_SITE_URL overrides it for a preview somewhere else.
export const SITE_URL = (
  process.env.NEXT_PUBLIC_SITE_URL ?? "https://gyotaku.app"
).replace(/\/$/, "");

export const REPO_URL = "https://github.com/xevrion/gyotaku";
export const RELEASES_URL = `${REPO_URL}/releases`;
export const AUTHOR_URL = "https://github.com/xevrion";

// The app version the site describes. Keep in step with the workspace
// version in ../Cargo.toml when a release goes out.
export const APP_VERSION = "0.1.6";

export const TITLE = "gyotaku: search your screenshots by text, offline";
// Written to fit a search result snippet (about 155 characters).
export const DESCRIPTION =
  "Search your screenshots by the text in them. Free, open source and offline, gyotaku finds any screenshot from a word you remember. macOS, Windows, Linux.";
