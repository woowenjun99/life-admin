import type { Metadata } from "next";
import type { ReactNode } from "react";

import { AuthProvider } from "@/components/auth/auth-provider";
import { PwaRegistration } from "@/components/pwa/pwa-registration";

import "./globals.css";

export const metadata: Metadata = {
  title: "Life Inbox — One clear next action",
  description:
    "A personal life-admin agent that turns life clutter into a calm, practical plan.",
  applicationName: "Life Inbox",
  manifest: "/manifest.webmanifest",
  appleWebApp: {
    capable: true,
    statusBarStyle: "default",
    title: "Life Inbox",
  },
  icons: {
    icon: "/icon.svg",
    apple: "/icon.svg",
  },
};

export default function RootLayout({
  children,
}: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <AuthProvider>
          <PwaRegistration />
          {children}
        </AuthProvider>
      </body>
    </html>
  );
}
