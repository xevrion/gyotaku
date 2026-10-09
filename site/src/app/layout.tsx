import type { Metadata, Viewport } from "next";
import { IBM_Plex_Mono, Inter } from "next/font/google";
import Script from "next/script";
import { Analytics } from "@vercel/analytics/next";
import { SmoothScroll } from "@/components/SmoothScroll";
import { themeScript } from "@/components/ThemeToggle";
import { AUTHOR_URL, DESCRIPTION, SITE_URL, TITLE } from "@/lib/site";
import "./globals.css";

// Inter for everything, with its optical size axis so the big headlines get
// the tighter display cut and body text the roomier text cut. IBM Plex Mono
// for commands: the app itself is set in IBM Plex.
const sans = Inter({
  variable: "--font-sans-face",
  subsets: ["latin"],
  axes: ["opsz"],
});

const mono = IBM_Plex_Mono({
  variable: "--font-mono-face",
  subsets: ["latin"],
  weight: ["400", "500"],
});

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: TITLE,
  description: DESCRIPTION,
  applicationName: "gyotaku",
  keywords: [
    "screenshot search",
    "search screenshots by text",
    "screenshot search app",
    "OCR screenshot search",
    "find text in screenshots",
    "search text in images offline",
    "macOS",
    "Linux",
    "Windows",
    "open source",
  ],
  authors: [{ name: "xevrion", url: AUTHOR_URL }],
  creator: "xevrion",
  category: "utilities",
  alternates: { canonical: "/" },
  openGraph: {
    type: "website",
    url: "/",
    siteName: "gyotaku",
    locale: "en_US",
    title: "gyotaku: Ctrl F for your screenshots",
    description: DESCRIPTION,
  },
  twitter: {
    card: "summary_large_image",
    title: "gyotaku: Ctrl F for your screenshots",
    description: DESCRIPTION,
  },
  robots: {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      "max-image-preview": "large",
      "max-snippet": -1,
      "max-video-preview": -1,
    },
  },
  appleWebApp: { title: "gyotaku" },
  formatDetection: { telephone: false },
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#fafaf9" },
    { media: "(prefers-color-scheme: dark)", color: "#0a0a0b" },
  ],
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      className={`${sans.variable} ${mono.variable}`}
      suppressHydrationWarning
    >
      <head>
        {/* Before first paint, so a saved theme never flashes the other one. */}
        <Script id="theme" strategy="beforeInteractive">
          {themeScript}
        </Script>
      </head>
      <body className="min-h-dvh">
        <SmoothScroll />
        {children}
        <Analytics />
      </body>
    </html>
  );
}
