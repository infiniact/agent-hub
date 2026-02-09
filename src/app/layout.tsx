import type { Metadata } from "next";
import { Space_Grotesk, Noto_Sans } from "next/font/google";
import "./globals.css";
import { Providers } from "./providers";

const spaceGrotesk = Space_Grotesk({
  variable: "--font-display",
  subsets: ["latin"],
  weight: ["300", "400", "500", "600", "700"],
});

const notoSans = Noto_Sans({
  variable: "--font-body",
  subsets: ["latin"],
  weight: ["400", "500", "600"],
});

export const metadata: Metadata = {
  title: "IAAgentHub",
  description: "AI Agent Management Desktop Application",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark">
      <body
        className={`${spaceGrotesk.variable} ${notoSans.variable} antialiased bg-background-light dark:bg-background-dark text-slate-900 dark:text-gray-100 font-display h-screen flex flex-col overflow-hidden`}
      >
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
