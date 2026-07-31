import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

import { InboxList, inboxItemLabel } from "./inbox-list";

test("inbox item labels do not reveal capture content", () => {
  expect(inboxItemLabel("text")).toBe("Text capture");
  expect(inboxItemLabel("image")).toBe("Image capture");
  expect(inboxItemLabel("pdf")).toBe("PDF capture");
});

test("Inbox list renders loading, empty, retry, and status states", () => {
  const retry = () => undefined;
  expect(
    renderToStaticMarkup(
      <InboxList onRetry={retry} state={{ status: "loading" }} />,
    ),
  ).toContain("Loading your private captures");
  expect(
    renderToStaticMarkup(
      <InboxList onRetry={retry} state={{ status: "ready", items: [] }} />,
    ),
  ).toContain("will appear here after you add one");
  expect(
    renderToStaticMarkup(
      <InboxList onRetry={retry} state={{ status: "error" }} />,
    ),
  ).toContain("Retry");
  expect(
    renderToStaticMarkup(
      <InboxList
        onRetry={retry}
        state={{
          status: "ready",
          items: [
            {
              id: "item-123",
              planId: "plan-123",
              sourceType: "pdf",
              status: "planned",
              canRetryExtraction: false,
              createdAt: "2026-07-30T00:00:00Z",
              updatedAt: "2026-07-30T00:00:00Z",
            },
          ],
        }}
      />,
    ),
  ).toContain("PDF capture");
  expect(
    renderToStaticMarkup(
      <InboxList
        onRetry={retry}
        state={{
          status: "ready",
          items: [
            {
              id: "item-123",
              planId: "plan-123",
              sourceType: "pdf",
              status: "planned",
              canRetryExtraction: false,
              createdAt: "2026-07-30T00:00:00Z",
              updatedAt: "2026-07-30T00:00:00Z",
            },
          ],
        }}
      />,
    ),
  ).toContain('href="/plans/plan-123"');

  const unsupportedPdf = renderToStaticMarkup(
    <InboxList
      onRetry={retry}
      state={{
        status: "ready",
        items: [
          {
            id: "item-456",
            sourceType: "pdf",
            status: "captured",
            canRetryExtraction: false,
            createdAt: "2026-07-30T00:00:00Z",
            updatedAt: "2026-07-30T00:00:00Z",
          },
        ],
      }}
    />,
  );
  expect(unsupportedPdf).not.toContain("Retry sorting");
});
