import { useSyncExternalStore } from "react";

/** The colour scheme a person chose; `auto` follows the system's. */
export type Theme = "auto" | "light" | "dark";

export const THEMES: readonly Theme[] = ["auto", "light", "dark"];

const KEY = "afterimage.theme";
const listeners = new Set<() => void>();

function stored(): Theme {
  const value = localStorage.getItem(KEY);
  return value === "light" || value === "dark" ? value : "auto";
}

/** Puts the chosen scheme on the root element, which the stylesheet's
 * light-dark() colours resolve against. Called before the first render, so a
 * dark page never flashes light. */
export function applyTheme(theme: Theme = stored()) {
  if (theme === "auto") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.dataset.theme = theme;
}

export function setTheme(theme: Theme) {
  if (theme === "auto") localStorage.removeItem(KEY);
  else localStorage.setItem(KEY, theme);
  applyTheme(theme);
  for (const listener of listeners) listener();
}

export function useTheme(): Theme {
  return useSyncExternalStore((listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, stored);
}
