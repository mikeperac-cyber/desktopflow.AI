import type { ThemePreference } from "../types/settings";

export function applyTheme(theme: ThemePreference): void {
  const resolved =
    theme === "system"
      ? window.matchMedia("(prefers-color-scheme: light)").matches
        ? "light"
        : "dark"
      : theme;

  document.documentElement.dataset.theme = resolved;
}

export function validateShortcut(shortcut: string): string | null {
  const value = shortcut.trim();
  if (!value) {
    return "Enter a keyboard shortcut.";
  }
  if (value.length > 64) {
    return "Shortcuts must be 64 characters or fewer.";
  }
  if (!value.includes("+") && !/^(esc|escape|f\d{1,2})$/i.test(value)) {
    return "Use a modifier and key, such as Alt + Space.";
  }
  return null;
}
