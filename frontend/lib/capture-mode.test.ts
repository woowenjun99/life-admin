import { expect, test } from "bun:test";

import { captureModeFromSearchParam } from "./capture-mode";

test("captureModeFromSearchParam accepts only supported capture modes", () => {
  expect(captureModeFromSearchParam("text")).toBe("text");
  expect(captureModeFromSearchParam("file")).toBe("file");
  expect(captureModeFromSearchParam("voice")).toBeNull();
  expect(captureModeFromSearchParam(null)).toBeNull();
});
