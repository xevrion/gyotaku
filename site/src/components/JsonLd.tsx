import {
  APP_VERSION,
  AUTHOR_URL,
  DESCRIPTION,
  RELEASES_URL,
  REPO_URL,
  SITE_URL,
} from "@/lib/site";

// Structured data for search engines: what the app is, that it's free, where
// to get it and who made it. No ratings, since there are none to cite.

const AUTHOR = {
  "@type": "Person",
  "@id": `${SITE_URL}/#author`,
  name: "xevrion",
  url: AUTHOR_URL,
  sameAs: [AUTHOR_URL, "https://x.com/xevrion_the1"],
};

// The features as they are in the app today, for featureList.
const FEATURES = [
  "Search every screenshot by the text inside it, as you type",
  "Matched words highlighted where they sit in the image",
  "Finds words the OCR misread, after every exact match",
  "Filters by folder and date",
  "Folds bursts of near-identical screenshots into one",
  "Copy the text of a whole screenshot or just the lines you select",
  "Move screenshots to the trash in bulk, with undo",
  "Reads English and other Latin-script languages, Chinese, Japanese and vertical text, with Hindi, Marathi, Nepali, Russian, Ukrainian and other Cyrillic languages optional",
  "Runs fully offline, with no account or telemetry",
];

export const SITE_GRAPH = [
  {
    "@type": "SoftwareApplication",
    "@id": `${SITE_URL}/#app`,
    name: "gyotaku",
    description: DESCRIPTION,
    url: SITE_URL,
    image: `${SITE_URL}/opengraph-image`,
    screenshot: `${SITE_URL}/opengraph-image`,
    applicationCategory: "UtilitiesApplication",
    applicationSubCategory: "Screenshot search",
    operatingSystem: "macOS, Windows, Linux",
    softwareVersion: APP_VERSION,
    downloadUrl: RELEASES_URL,
    installUrl: `${SITE_URL}/#install`,
    featureList: FEATURES,
    license: "https://www.gnu.org/licenses/gpl-3.0.html",
    isAccessibleForFree: true,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    author: { "@id": `${SITE_URL}/#author` },
    sameAs: [REPO_URL],
  },
  {
    "@type": "SoftwareSourceCode",
    "@id": `${REPO_URL}#source`,
    name: "gyotaku",
    codeRepository: REPO_URL,
    programmingLanguage: "Rust",
    license: "https://www.gnu.org/licenses/gpl-3.0.html",
    targetProduct: { "@id": `${SITE_URL}/#app` },
    author: { "@id": `${SITE_URL}/#author` },
  },
  {
    "@type": "WebSite",
    "@id": `${SITE_URL}/#website`,
    name: "gyotaku",
    url: SITE_URL,
    inLanguage: "en",
    publisher: { "@id": `${SITE_URL}/#author` },
  },
  AUTHOR,
];

export function faqPage(items: { q: string; text: string }[]) {
  return {
    "@type": "FAQPage",
    "@id": `${SITE_URL}/#faq`,
    mainEntity: items.map((item) => ({
      "@type": "Question",
      name: item.q,
      acceptedAnswer: { "@type": "Answer", text: item.text },
    })),
  };
}

export function breadcrumbs(trail: { name: string; path: string }[]) {
  return {
    "@type": "BreadcrumbList",
    itemListElement: trail.map((t, i) => ({
      "@type": "ListItem",
      position: i + 1,
      name: t.name,
      item: `${SITE_URL}${t.path}`,
    })),
  };
}

export function JsonLd({ graph }: { graph: object[] }) {
  return (
    <script
      type="application/ld+json"
      // `<` is escaped so the JSON can never close the script tag early.
      dangerouslySetInnerHTML={{
        __html: JSON.stringify({ "@context": "https://schema.org", "@graph": graph }).replace(
          /</g,
          "\\u003c",
        ),
      }}
    />
  );
}
