import { describe, expect, test } from "bun:test";

const stylesheet = await Bun.file(
  new URL("./globals.css", import.meta.url),
).text();

function mobileStyles() {
  const start = stylesheet.indexOf("@media (max-width: 540px)");
  return start === -1 ? "" : stylesheet.slice(start);
}

describe("workspace navigation on small screens", () => {
  test("gives every action an equal-width track below the brand", () => {
    const styles = mobileStyles();

    expect(styles).toContain(".workspace-nav-actions {");
    expect(styles).toContain(
      "grid-template-columns: repeat(3, minmax(0, 1fr))",
    );
    expect(styles).toContain(".workspace-nav-actions .button {");
    expect(styles).toContain("width: 100%");
    expect(styles).toContain(".workspace-nav {");
    expect(styles).toContain("flex-direction: column");
  });
});
