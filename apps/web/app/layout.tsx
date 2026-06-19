import type { Metadata } from "next";
import localFont from "next/font/local";
import { Fraunces, IBM_Plex_Mono } from "next/font/google";
import "./globals.css";
import { TRPCReactProvider } from "../trpc/client";

const geistSans = localFont({
  src: "./fonts/GeistVF.woff",
  variable: "--font-geist-sans",
});

const fraunces = Fraunces({
  subsets: ["latin"],
  variable: "--font-fraunces",
  axes: ["opsz", "SOFT"],
  display: "swap",
});

const plexMono = IBM_Plex_Mono({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  variable: "--font-plex-mono",
  display: "swap",
});

export const metadata: Metadata = {
  title: "SoundGrammy",
  description: "Your personal music library, tuned in from Telegram",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark">
      <body
        className={`${geistSans.variable} ${fraunces.variable} ${plexMono.variable} grain antialiased w-screen h-screen overflow-hidden`}
      >
        <TRPCReactProvider>{children}</TRPCReactProvider>
      </body>
    </html>
  );
}
