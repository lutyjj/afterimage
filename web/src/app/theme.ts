import { useSyncExternalStore } from "react";

/** The colour scheme a person chose; `auto` follows the system's. */
export type Theme = "auto" | "light" | "dark";

export const THEMES: readonly Theme[] = ["auto", "light", "dark"];

/** Where the choice is kept in this browser; index.html reads it too, to set the
 * theme before the first paint. */
const KEY = "afterimage.theme";
const listeners = new Set<() => void>();

/** Puts the scheme on the root element, which the stylesheet's light-dark()
 * colours resolve against. */
function apply(theme: Theme) {
  if (theme === "auto") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.dataset.theme = theme;
}

/** Chooses a scheme, kept in this browser where it allows. */
export function setTheme(theme: Theme) {
  try {
    if (theme === "auto") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, theme);
  } catch {
    // The choice holds for this page only.
  }
  apply(theme);
  for (const listener of listeners) listener();
}

/** The scheme in force: what the root element carries, which index.html sets
 * from the stored choice before the first paint. */
function current(): Theme {
  const theme = document.documentElement.dataset.theme;
  return theme === "light" || theme === "dark" ? theme : "auto";
}

export function useTheme(): Theme {
  return useSyncExternalStore((listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, current);
}
