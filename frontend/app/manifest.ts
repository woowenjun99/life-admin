import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Life Inbox",
    short_name: "Life Inbox",
    description:
      "A calm private workspace for turning life clutter into one clear next action.",
    start_url: "/today",
    scope: "/",
    display: "standalone",
    background_color: "#f6f4ee",
    theme_color: "#183c36",
    icons: [
      {
        src: "/icon.svg",
        sizes: "any",
        type: "image/svg+xml",
        purpose: "maskable",
      },
    ],
  };
}
