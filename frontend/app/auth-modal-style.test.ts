import { describe, expect, test } from "bun:test";

const stylesheet = await Bun.file(
  new URL("./globals.css", import.meta.url),
).text();

function rulesFor(selector: string) {
  const start = stylesheet.indexOf(`${selector} {`);
  const end = stylesheet.indexOf("}", start);

  return start === -1 || end === -1 ? "" : stylesheet.slice(start, end);
}

describe("auth modal layout", () => {
  test("keeps the dialog compact and scrollable within the viewport", () => {
    const modalRules = rulesFor(".auth-modal");

    expect(modalRules).toContain("max-height: calc(100svh - 2rem)");
    expect(modalRules).toContain("overflow-y: auto");
    expect(rulesFor(".auth-modal .auth-copy")).toContain("margin-top: 1.75rem");
    expect(rulesFor(".auth-modal .auth-copy h1")).toContain("max-width: 14ch");
  });
});
