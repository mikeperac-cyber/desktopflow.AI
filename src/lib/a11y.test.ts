import { describe, expect, it } from "vitest";

import cssContent from "../App.css?raw";

describe("Accessibility & Motion CSS Rules", () => {
  it("includes prefers-reduced-motion media query disabling animations", () => {
    expect(cssContent).toContain("@media (prefers-reduced-motion: reduce)");
    expect(cssContent).toContain("animation-duration: 0.01ms !important;");
    expect(cssContent).toContain(".overlay-shell {");
    expect(cssContent).toContain("animation: none !important;");
  });

  it("includes forced-colors active media query for Windows High Contrast Mode", () => {
    expect(cssContent).toContain("@media (forced-colors: active)");
    expect(cssContent).toContain("CanvasText");
    expect(cssContent).toContain("ButtonBorder");
    expect(cssContent).toContain("Highlight");
    expect(cssContent).toContain("HighlightText");
  });

  it("defines focus-visible outlines for keyboard navigation", () => {
    expect(cssContent).toContain("button:focus-visible");
    expect(cssContent).toContain("input:focus-visible");
    expect(cssContent).toContain("select:focus-visible");
    expect(cssContent).toContain("textarea:focus-visible");
    expect(cssContent).toContain("outline: 2px solid var(--accent);");
  });

  it("defines dark and light design system color tokens", () => {
    expect(cssContent).toContain("--bg: #11161d;");
    expect(cssContent).toContain("--accent: #2997ff;");
    expect(cssContent).toContain("--bg: #f8fafc;");
    expect(cssContent).toContain("--accent: #087fe7;");
  });
});
