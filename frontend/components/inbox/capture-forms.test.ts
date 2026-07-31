import { expect, test } from "bun:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { ApiError } from "@/lib/api";

import { captureErrorMessage } from "@/lib/capture";

import { CaptureLauncher } from "./capture-launcher";

test("captureErrorMessage keeps private file-storage failure messages actionable", () => {
  expect(
    captureErrorMessage(
      new ApiError(503, "internal detail", "STORAGE_UNAVAILABLE"),
    ),
  ).toBe(
    "Private file storage is temporarily unavailable. Please try again later.",
  );
  expect(captureErrorMessage(new Error("network failed"))).toBe(
    "Something went wrong. Please try again.",
  );
});

test("first-task launcher explains the private capture-to-next-action flow", () => {
  const markup = renderToStaticMarkup(
    createElement(CaptureLauncher, {
      onCaptureFile: () => undefined,
      onCaptureText: () => undefined,
      variant: "first_task",
    }),
  );

  expect(markup).toContain("Your first task");
  expect(markup).toContain("Capture something");
  expect(markup).toContain("Upload a file");
  expect(markup).toContain("Review it");
  expect(markup).toContain("Get one next action");
  expect(markup).toContain(
    "Nothing happens outside Life Inbox unless you choose it.",
  );
  expect(markup).toContain('aria-label="Your first Life Inbox task"');
  expect(markup).toContain("<ol>");
});
