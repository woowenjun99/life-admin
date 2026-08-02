import { afterEach, expect, test } from "bun:test";
import { NextRequest } from "next/server";

Object.assign(process.env, {
  BACKEND_INTERNAL_URL: "http://backend:3001/internal",
  NEXT_PUBLIC_FIREBASE_API_KEY: "test-api-key",
  NEXT_PUBLIC_FIREBASE_APP_ID: "test-app-id",
  NEXT_PUBLIC_FIREBASE_AUTH_DOMAIN: "test.firebaseapp.com",
  NEXT_PUBLIC_FIREBASE_MESSAGING_SENDER_ID: "1234567890",
  NEXT_PUBLIC_FIREBASE_PROJECT_ID: "test-project",
  NEXT_PUBLIC_FIREBASE_STORAGE_BUCKET: "test-project.appspot.com",
});

const originalFetch = globalThis.fetch;
const { config, proxy } = await import("./proxy");

afterEach(() => {
  globalThis.fetch = originalFetch;
});

test("matches only the home page and authenticated workspace routes", () => {
  expect(config.matcher).toEqual([
    "/",
    "/today/:path*",
    "/plans/:path*",
    "/inbox/:path*",
  ]);
});

test("redirects a private route without a session to the home sign-in flow", async () => {
  const response = await proxy(new NextRequest("https://life.test/today"));

  expect(response.status).toBe(307);
  expect(response.headers.get("location")).toBe(
    "https://life.test/?auth=sign-in",
  );
});

test("allows a private route with a valid session and forwards only that cookie", async () => {
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return new Response(null, { status: 204 });
  }) as typeof fetch;

  const response = await proxy(
    new NextRequest("https://life.test/plans/plan-123", {
      headers: { cookie: "tracking=private; life_inbox_session=valid-session" },
    }),
  );

  expect(response.status).toBe(200);
  expect(request?.input.toString()).toBe(
    "http://backend:3001/internal/api/v1/auth/session",
  );
  expect(new Headers(request?.init?.headers).get("Cookie")).toBe(
    "life_inbox_session=valid-session",
  );
});

test("redirects an authenticated home request to Today", async () => {
  globalThis.fetch = (async () =>
    new Response(null, { status: 204 })) as unknown as typeof fetch;

  const response = await proxy(
    new NextRequest("https://life.test/", {
      headers: { cookie: "life_inbox_session=valid-session" },
    }),
  );

  expect(response.status).toBe(307);
  expect(response.headers.get("location")).toBe("https://life.test/today");
});

test("clears an invalid session before redirecting a private route", async () => {
  globalThis.fetch = (async () =>
    new Response(null, { status: 401 })) as unknown as typeof fetch;

  const response = await proxy(
    new NextRequest("https://life.test/inbox/item-123/review", {
      headers: { cookie: "life_inbox_session=expired-session" },
    }),
  );

  expect(response.status).toBe(307);
  expect(response.headers.get("location")).toBe(
    "https://life.test/?auth=sign-in",
  );
  expect(response.headers.get("set-cookie")).toContain(
    "life_inbox_session=; Path=/; Max-Age=0",
  );
});

test("fails closed for private routes when session validation is unavailable without clearing the cookie", async () => {
  globalThis.fetch = (async () => {
    throw new Error("backend unavailable");
  }) as unknown as typeof fetch;

  const response = await proxy(
    new NextRequest("https://life.test/today", {
      headers: { cookie: "life_inbox_session=still-valid" },
    }),
  );

  expect(response.status).toBe(307);
  expect(response.headers.get("location")).toBe(
    "https://life.test/?auth=sign-in",
  );
  expect(response.headers.get("set-cookie")).toBeNull();
});
