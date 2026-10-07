import { describe, expect, it } from "vitest";

import { applyTheme, validateShortcut } from "./theme";

describe("validateShortcut", () => {
  it("accepts modifier shortcuts and emergency keys", () => {
    expect(validateShortcut("Alt + Space")).toBeNull();
    expect(validateShortcut("Win + Shift + A")).toBeNull();
    expect(validateShortcut("Esc")).toBeNull();
  });

  it("rejects an ambiguous single key", () => {
    expect(validateShortcut("A")).toMatch(/modifier/i);
  });
});

describe("applyTheme", () => {
  it("sets document theme to dark and light directly", () => {
    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");

    applyTheme("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("resolves system theme using prefers-color-scheme matchMedia", () => {
    applyTheme("system");
    expect(["dark", "light"]).toContain(document.documentElement.dataset.theme);
  });
});
