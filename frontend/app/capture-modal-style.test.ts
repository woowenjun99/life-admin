import { describe, expect, test } from "bun:test";

const stylesheet = await Bun.file(
  new URL("./globals.css", import.meta.url),
).text();
const phoneStyles = stylesheet.slice(
  stylesheet.indexOf("@media (max-width: 540px)"),
);

function rulesFor(selector: string) {
  const start = phoneStyles.indexOf(`${selector} {`);
  const end = phoneStyles.indexOf("}", start);

  return start === -1 || end === -1 ? "" : phoneStyles.slice(start, end);
}

describe("phone capture modal", () => {
  test("uses a viewport-safe bottom sheet with touch-friendly controls", () => {
    expect(rulesFor(".capture-modal-backdrop")).toContain(
      "place-items: end stretch",
    );
    expect(rulesFor(".capture-modal")).toContain(
      "max-height: calc(100dvh - env(safe-area-inset-top))",
    );
    expect(rulesFor(".capture-modal")).toContain(
      "border-radius: 1.25rem 1.25rem 0 0",
    );
    expect(rulesFor(".capture-modal-close")).toContain("width: 2.75rem");
    expect(rulesFor(".capture-mode-button")).toContain("min-height: 2.75rem");
    expect(rulesFor(".capture-launcher-actions")).toContain(
      "flex-direction: column",
    );
  });
});
