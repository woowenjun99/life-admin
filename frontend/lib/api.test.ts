import { expect, test } from "bun:test";

import {
  authorizationHeaders,
  createTextCapture,
  parseCurrentUser,
  uploadFileCapture,
  validateCaptureFile,
} from "./api";

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

test("createTextCapture sends a bearer JSON request and parses the safe item response", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json({
      inboxItem: {
        id: "item-123",
        sourceType: "text",
        status: "captured",
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
      },
    });
  }) as typeof fetch;

  try {
    const item = await createTextCapture(
      { getIdToken: async () => "firebase-id-token" },
      "Renew passport",
    );

    expect(item.sourceType).toBe("text");
    expect(request?.input).toBe("/api/v1/inbox-items");
    const headers = new Headers(request?.init?.headers);
    expect(headers.get("Authorization")).toBe("Bearer firebase-id-token");
    expect(headers.get("Content-Type")).toBe("application/json");
    expect(request?.init?.body).toBe('{"text":"Renew passport"}');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("uploadFileCapture sends multipart without a browser-selected content type header", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json({
      inboxItem: {
        id: "item-456",
        sourceType: "pdf",
        status: "captured",
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
      },
    });
  }) as typeof fetch;

  try {
    await uploadFileCapture(
      { getIdToken: async () => "firebase-id-token" },
      new File(["%PDF-1.7"], "letter.pdf", { type: "application/pdf" }),
    );

    expect(request?.input).toBe("/api/v1/inbox-items/files");
    const headers = new Headers(request?.init?.headers);
    expect(headers.get("Authorization")).toBe("Bearer firebase-id-token");
    expect(headers.get("Content-Type")).toBeNull();
    expect(request?.init?.body).toBeInstanceOf(FormData);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("upload errors keep the server error code for capture state messaging", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () =>
    Response.json(
      {
        error: {
          code: "FILE_SCAN_UNAVAILABLE",
          message: "File scanning is temporarily unavailable.",
        },
      },
      { status: 503 },
    )) as unknown as typeof fetch;

  try {
    await expect(
      uploadFileCapture(
        { getIdToken: async () => "firebase-id-token" },
        new File(["%PDF-1.7"], "letter.pdf", { type: "application/pdf" }),
      ),
    ).rejects.toMatchObject({
      status: 503,
      code: "FILE_SCAN_UNAVAILABLE",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("validateCaptureFile gives immediate type and size feedback", () => {
  expect(validateCaptureFile({ type: "text/plain", size: 3 })).toBe(
    "Choose a PDF, JPEG, or PNG file.",
  );
  expect(
    validateCaptureFile({
      type: "application/pdf",
      size: 10 * 1024 * 1024 + 1,
    }),
  ).toBe("Files must not exceed 10 MiB.");
  expect(validateCaptureFile({ type: "image/png", size: 10 })).toBeNull();
});
