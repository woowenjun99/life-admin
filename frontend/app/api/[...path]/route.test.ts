import { expect, test } from "bun:test";

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
