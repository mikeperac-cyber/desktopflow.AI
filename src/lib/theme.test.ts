import { describe, expect, it } from "vitest";

import { validateShortcut } from "./theme";

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
