import { expect, test } from "bun:test";
import { MAX_CAPTURE_FILE_BYTES } from "./lib/api";
import nextConfig from "./next.config";

test("the Next proxy permits multipart overhead above the accepted file size", () => {
  const bodyLimit = nextConfig.experimental?.proxyClientMaxBodySize;

  if (typeof bodyLimit !== "number") {
    throw new Error("proxyClientMaxBodySize must be a byte limit");
  }

  expect(bodyLimit).toBeGreaterThan(MAX_CAPTURE_FILE_BYTES);
});
