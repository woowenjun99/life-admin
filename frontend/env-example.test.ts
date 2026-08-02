import { expect, test } from "bun:test";

const publicEnvExample = await Bun.file(
  new URL("./.env.example", import.meta.url),
).text();
const localLauncher = await Bun.file(
  new URL("../scripts/start-local.sh", import.meta.url),
).text();

test("the frontend environment example exposes only Firebase Web configuration", () => {
  for (const key of [
    "NEXT_PUBLIC_FIREBASE_API_KEY=",
    "NEXT_PUBLIC_FIREBASE_AUTH_DOMAIN=",
    "NEXT_PUBLIC_FIREBASE_PROJECT_ID=",
    "NEXT_PUBLIC_FIREBASE_STORAGE_BUCKET=",
    "NEXT_PUBLIC_FIREBASE_MESSAGING_SENDER_ID=",
    "NEXT_PUBLIC_FIREBASE_APP_ID=",
    "NEXT_PUBLIC_FIREBASE_VAPID_KEY=",
  ]) {
    expect(publicEnvExample).toContain(key);
  }

  expect(publicEnvExample).not.toContain("BACKEND_INTERNAL_URL");
  expect(publicEnvExample).not.toContain("FIREBASE_SERVICE_ACCOUNT_JSON");
  expect(publicEnvExample).not.toContain("OPENAI_API_KEY");
});

test("the local launcher keeps its backend target server-only", () => {
  expect(localLauncher).toContain(
    'BACKEND_INTERNAL_URL="http://127.0.0.1:$BACKEND_PORT"',
  );
});
