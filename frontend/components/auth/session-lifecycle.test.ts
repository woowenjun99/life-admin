import { expect, test } from "bun:test";

import { clearSessionBeforeFirebaseSignOut } from "./session-lifecycle";

test("clears the private session before signing out of Firebase", async () => {
  const calls: string[] = [];

  await clearSessionBeforeFirebaseSignOut(
    async () => {
      calls.push("clear-session");
    },
    async () => {
      calls.push("firebase-sign-out");
    },
  );

  expect(calls).toEqual(["clear-session", "firebase-sign-out"]);
});

test("keeps Firebase signed in when its matching private session cannot be cleared", async () => {
  const calls: string[] = [];
  const clearError = new Error("session backend unavailable");

  await expect(
    clearSessionBeforeFirebaseSignOut(
      async () => {
        calls.push("clear-session");
        throw clearError;
      },
      async () => {
        calls.push("firebase-sign-out");
      },
    ),
  ).rejects.toBe(clearError);

  expect(calls).toEqual(["clear-session"]);
});
