"use client";

import Link from "next/link";

import { useWorkspaceUser } from "@/components/auth/private-workspace";

export function TodayContent() {
  const currentUser = useWorkspaceUser();

  return (
    <section className="workspace-panel">
      <p className="eyebrow">Your private workspace</p>
      <h1>Your Life Inbox is ready.</h1>
      <p className="workspace-intro">
        Signed in as <strong>{currentUser.email}</strong>. Your captures and
        plans will stay connected to this account.
      </p>

      <div className="workspace-empty-state">
        <span aria-hidden="true" className="workspace-empty-icon">
          +
        </span>
        <div className="workspace-capture-copy">
          <p className="workspace-empty-kicker">Capture</p>
          <h2>Capture the first thing on your mind.</h2>
          <p>
            Save a private note, PDF, JPEG, or PNG. File captures are scanned
            before they are stored.
          </p>
          <Link
            className="button button-primary workspace-capture-button"
            href="/inbox"
          >
            Open Inbox capture <span aria-hidden="true">↗</span>
          </Link>
        </div>
      </div>
    </section>
  );
}
