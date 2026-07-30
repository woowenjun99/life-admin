"use client";

import type { InboxItem } from "@/lib/api";

export type InboxListState =
  | { status: "loading" }
  | { status: "ready"; items: InboxItem[] }
  | { status: "error" };

export function inboxItemLabel(sourceType: InboxItem["sourceType"]): string {
  switch (sourceType) {
    case "text":
      return "Text capture";
    case "image":
      return "Image capture";
    case "pdf":
      return "PDF capture";
  }
}

function inboxItemStatus(status: InboxItem["status"]): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

function capturedAt(timestamp: string): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.valueOf())) {
    return "Captured recently";
  }

  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  }).format(date);
}

export function InboxList({
  onRetry,
  state,
}: {
  onRetry(): void;
  state: InboxListState;
}) {
  return (
    <section
      aria-labelledby="inbox-list-heading"
      className="inbox-list-section"
    >
      <div className="inbox-list-heading">
        <div>
          <p className="workspace-empty-kicker">Your private Inbox</p>
          <h2 id="inbox-list-heading">Saved captures</h2>
        </div>
        {state.status === "ready" ? (
          <p className="inbox-list-count">
            {state.items.length === 1
              ? "1 capture"
              : `${state.items.length} captures`}
          </p>
        ) : null}
      </div>

      {state.status === "loading" ? (
        <p aria-busy="true" className="inbox-list-notice">
          Loading your private captures…
        </p>
      ) : null}

      {state.status === "error" ? (
        <div className="inbox-list-notice inbox-list-error" role="alert">
          <p>We could not load your Inbox. Please try again.</p>
          <button
            className="button button-ghost"
            onClick={onRetry}
            type="button"
          >
            Retry
          </button>
        </div>
      ) : null}

      {state.status === "ready" && state.items.length === 0 ? (
        <p className="inbox-list-notice">
          Your saved captures will appear here after you add one.
        </p>
      ) : null}

      {state.status === "ready" && state.items.length > 0 ? (
        <ul className="inbox-items-list">
          {state.items.map((item) => (
            <li className="inbox-item-card" key={item.id}>
              <div>
                <p className="inbox-item-label">
                  {inboxItemLabel(item.sourceType)}
                </p>
                <time dateTime={item.createdAt}>
                  Captured {capturedAt(item.createdAt)}
                </time>
              </div>
              <span className="inbox-item-status">
                {inboxItemStatus(item.status)}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
