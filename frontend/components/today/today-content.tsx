"use client";

import { InboxContent } from "@/components/inbox/inbox-content";

export function TodayContent() {
  return (
    <section className="workspace-panel inbox-panel">
      <p className="eyebrow">Your private workspace</p>
      <h1 className="workspace-heading">Your Life Inbox is ready.</h1>
      <InboxContent />
    </section>
  );
}
