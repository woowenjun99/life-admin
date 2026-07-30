import { expect, test } from "bun:test";

import { ApiError } from "@/lib/api";

import { captureErrorMessage } from "@/lib/capture";

test("captureErrorMessage keeps private file-storage failure messages actionable", () => {
  expect(
    captureErrorMessage(
      new ApiError(503, "internal detail", "STORAGE_UNAVAILABLE"),
    ),
  ).toBe("Private file storage is temporarily unavailable. Please try again later.");
  expect(captureErrorMessage(new Error("network failed"))).toBe(
    "Something went wrong. Please try again.",
  );
});
