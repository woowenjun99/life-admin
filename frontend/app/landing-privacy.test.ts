import { expect, test } from "bun:test";

const pageSource = await Bun.file(
  new URL("./page.tsx", import.meta.url),
).text();
const stylesheet = await Bun.file(
  new URL("./globals.css", import.meta.url),
).text();
const readmeSource = await Bun.file(
  new URL("../../README.md", import.meta.url),
).text();
const todoSource = await Bun.file(
  new URL("../../TODO.md", import.meta.url),
).text();

test("the landing page gives an accurate, accessible privacy summary", () => {
  expect(pageSource).toContain('aria-labelledby="privacy-heading"');
  expect(pageSource).toContain('id="privacy-heading"');
  expect(pageSource).toContain("Privacy at a glance");
  expect(pageSource).toContain("Your workspace is tied to your sign-in");
  expect(pageSource).toContain("Images are never sent.");
  expect(pageSource).toMatch(/Life Inbox never\s+contacts other people/);
  expect(pageSource).toMatch(/If you turn on\s+alerts/);
  expect(pageSource).not.toContain("never takes action outside the app.");
  expect(readmeSource).toContain("If you opt in to alerts");
  expect(readmeSource).not.toContain(
    "Life Inbox does not send\n  messages, make purchases",
  );
});

test("production readiness stays unchecked until the deployed flow is recorded", () => {
  expect(todoSource).toContain("- [ ] Deploy a stable demo environment.");
  expect(todoSource).toContain(
    "- [ ] Verify the deployed capture → review → Plan → complete flow in production.",
  );
});

test("the privacy summary aligns with the landing page and stacks on tablets", () => {
  expect(stylesheet).toContain(".privacy-section {");
  expect(stylesheet).toContain("grid-template-columns: 0.75fr 1.25fr");

  const tabletStyles = stylesheet.slice(
    stylesheet.indexOf("@media (max-width: 800px)"),
  );
  expect(tabletStyles).toContain(".privacy-section {");
  expect(tabletStyles).toContain("grid-template-columns: 1fr");
});
