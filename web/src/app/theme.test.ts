import { readFileSync } from "node:fs";

import { describe, expect, it, vi } from "vitest";

import { THEME_KEY, THEME_OPTIONS } from "./theme";

// The store reads and writes the page, which these tests have none of.
vi.mock("linen", () => ({ createThemeStore: () => ({}) }));

describe("the shell's early script", () => {
  it("reads the key and the schemes the theme store writes", () => {
    const shell = readFileSync(new URL("../../index.html", import.meta.url), "utf8");
    expect(shell).toContain(`localStorage.getItem("${THEME_KEY}")`);
    for (const { value } of THEME_OPTIONS.filter((option) => option.value !== "system")) {
      expect(shell).toContain(`theme === "${value}"`);
    }
  });
});
