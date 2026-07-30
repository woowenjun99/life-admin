import { expect, test } from "bun:test";

import { ApiError } from "@/lib/api";

import { captureErrorMessage } from "@/lib/capture";

test("captureErrorMessage keeps privacy-safe file failure messages actionable", () => {
  expect(
    captureErrorMessage(
      new ApiError(503, "internal detail", "FILE_SCAN_UNAVAILABLE"),
    ),
  ).toBe("File scanning is temporarily unavailable. Please try again later.");
  expect(captureErrorMessage(new Error("network failed"))).toBe(
    "Something went wrong. Please try again.",
  );
});
