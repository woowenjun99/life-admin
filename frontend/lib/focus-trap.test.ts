import { expect, test } from "bun:test";

import { focusTrapTargetIndex } from "./focus-trap";

test("wraps forward tabbing from the last dialog control to the first", () => {
  expect(focusTrapTargetIndex(4, 3, false)).toBe(0);
});

test("wraps reverse tabbing from the first dialog control to the last", () => {
  expect(focusTrapTargetIndex(4, 0, true)).toBe(3);
});

test("moves focus inside the dialog when it has escaped", () => {
  expect(focusTrapTargetIndex(4, -1, false)).toBe(0);
  expect(focusTrapTargetIndex(4, -1, true)).toBe(3);
});

test("does not interfere with tabbing between dialog controls", () => {
  expect(focusTrapTargetIndex(4, 1, false)).toBeNull();
  expect(focusTrapTargetIndex(4, 2, true)).toBeNull();
});
