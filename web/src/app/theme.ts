import { createThemeStore, type ThemePreference } from "linen";

/** Where this browser keeps the theme; index.html reads it too, to set the
 * theme before the first paint. */
export const THEME_KEY = "afterimage.theme";

/** The page's ground in each scheme, for the browser's own chrome. */
const BAR = { light: "#f7fafb", dark: "#0d1113" } as const;

export const { useTheme } = createThemeStore({ key: THEME_KEY, bar: BAR });

export const THEME_OPTIONS: readonly { value: ThemePreference; label: string }[] = [
  { value: "system", label: "Auto" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];
