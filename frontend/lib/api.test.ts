import { expect, test } from "bun:test";

import { authorizationHeaders, parseCurrentUser } from "./api";

test("authorizationHeaders preserves existing headers and attaches a bearer token", async () => {
  const headers = await authorizationHeaders(
    {
      getIdToken: async () => "firebase-id-token",
    },
    { "X-Request-Id": "request-123" },
  );

  expect(headers.get("Authorization")).toBe("Bearer firebase-id-token");
  expect(headers.get("X-Request-Id")).toBe("request-123");
});

test("parseCurrentUser rejects an invalid identity response", () => {
  expect(() => parseCurrentUser({ user: { uid: "user-123" } })).toThrow(
    "The workspace identity response was invalid.",
  );
});
