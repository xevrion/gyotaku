export const REPO_SLUG = "xevrion/gyotaku";
export const REPO = `https://github.com/${REPO_SLUG}`;

// Shown until the live count arrives from GitHub. Bump it now and then so
// the first paint isn't far behind.
export const SAVED_STARS = 146;

export const SPONSOR = "https://github.com/sponsors/xevrion";

// The Buttondown newsletter's username. The signup only renders once this is
// set, so the page never shows a form that posts nowhere. The app and the
// README link to gyotaku.app/#updates rather than here, so changing it never
// needs a release.
export const BUTTONDOWN: string | null = null;
