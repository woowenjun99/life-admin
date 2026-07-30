"use client";

import { useWorkspaceUser } from "@/components/auth/private-workspace";
import { InboxContent } from "@/components/inbox/inbox-content";

export function TodayContent() {
  const currentUser = useWorkspaceUser();

  return (
    <section className="workspace-panel inbox-panel">
      <p className="eyebrow">Your private workspace</p>
      <h1>Your Life Inbox is ready.</h1>
      <p className="workspace-intro">
        Signed in as <strong>{currentUser.email}</strong>. Your captures and
        plans will stay connected to this account.
      </p>
      <InboxContent />
    </section>
  );
}
