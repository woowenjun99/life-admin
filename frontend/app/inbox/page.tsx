import type { Metadata } from "next";

import { PrivateWorkspace } from "@/components/auth/private-workspace";
import { CaptureForms } from "@/components/inbox/capture-forms";

export const metadata: Metadata = {
  title: "Inbox capture — Life Inbox",
};

export default function InboxPage() {
  return (
    <PrivateWorkspace>
      <section className="workspace-panel inbox-panel">
        <p className="eyebrow">Private Inbox capture</p>
        <h1>Get it out of your head.</h1>
        <p className="workspace-intro">
          Capture a note or one supported file. This screen does not show,
          preview, or download saved files.
        </p>
        <CaptureForms />
      </section>
    </PrivateWorkspace>
  );
}
