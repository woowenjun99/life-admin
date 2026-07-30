import { expect, test } from "bun:test";

import { authenticationErrorMessage } from "./auth";

test("maps sign-in credential failures to a non-enumerating message", () => {
  expect(
    authenticationErrorMessage({ code: "auth/invalid-credential" }, "sign-in"),
  ).toBe("Email or password is incorrect.");
});

test("maps sign-up password failures to actionable feedback", () => {
  expect(
    authenticationErrorMessage({ code: "auth/weak-password" }, "sign-up"),
  ).toBe("Use a password with at least six characters.");
});

test("keeps unknown authentication errors generic", () => {
  expect(authenticationErrorMessage(new Error("network"), "sign-in")).toBe(
    "We could not sign you in. Please try again.",
  );
});
