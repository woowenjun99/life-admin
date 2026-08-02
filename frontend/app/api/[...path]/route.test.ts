import { expect, test } from "bun:test";
import { NextRequest } from "next/server";

Object.assign(process.env, {
  BACKEND_INTERNAL_URL: "http://127.0.0.1:3001",
  NEXT_PUBLIC_FIREBASE_API_KEY: "test-api-key",
  NEXT_PUBLIC_FIREBASE_APP_ID: "test-app-id",
  NEXT_PUBLIC_FIREBASE_AUTH_DOMAIN: "test.firebaseapp.com",
  NEXT_PUBLIC_FIREBASE_MESSAGING_SENDER_ID: "1234567890",
  NEXT_PUBLIC_FIREBASE_PROJECT_ID: "test-project",
  NEXT_PUBLIC_FIREBASE_STORAGE_BUCKET: "test-project.appspot.com",
});

const route = await import("./route");

test("proxies HEAD requests", () => {
  expect(route.HEAD).toBe(route.GET);
});

test("forwards a backend session cookie response", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () =>
    new Response(null, {
      status: 204,
      headers: {
        "set-cookie":
          "life_inbox_session=issued-session; Path=/; HttpOnly; SameSite=Lax",
      },
    })) as unknown as typeof fetch;

  try {
    const response = await route.POST(
      new NextRequest("http://localhost:3000/api/v1/auth/session", {
        method: "POST",
        body: '{"idToken":"firebase-id-token"}',
      }),
      { params: Promise.resolve({ path: ["v1", "auth", "session"] }) },
    );

    expect(response.status).toBe(204);
    expect(response.headers.get("set-cookie")).toBe(
      "life_inbox_session=issued-session; Path=/; HttpOnly; SameSite=Lax",
    );
  } finally {
    globalThis.fetch = originalFetch;
  }
});
