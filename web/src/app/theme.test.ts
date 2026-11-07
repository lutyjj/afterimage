import { readFileSync } from "node:fs";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setTheme, THEME_KEY, THEMES } from "./theme";

/** A root element with the one attribute the theme uses. */
function root() {
  const dataset: Record<string, string> = {};
  return { dataset, removeAttribute: (name: string) => name === "data-theme" && delete dataset.theme };
}

describe("setTheme", () => {
  let store: Map<string, string>;
  let element: ReturnType<typeof root>;

  beforeEach(() => {
    store = new Map();
    element = root();
    vi.stubGlobal("document", { documentElement: element });
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => store.set(key, value),
      removeItem: (key: string) => store.delete(key),
    });
  });

  afterEach(() => vi.unstubAllGlobals());

  it("keeps a chosen scheme and puts it on the root", () => {
    setTheme("dark");
    expect(store.get("afterimage.theme")).toBe("dark");
    expect(element.dataset.theme).toBe("dark");
  });

  it("forgets the choice when it follows the system again", () => {
    setTheme("light");
    setTheme("auto");
    expect(store.has("afterimage.theme")).toBe(false);
    expect(element.dataset.theme).toBeUndefined();
  });

  it("still applies the scheme where the browser refuses storage", () => {
    vi.stubGlobal("localStorage", {
      setItem: () => {
        throw new DOMException("refused", "SecurityError");
      },
    });
    setTheme("dark");
    expect(element.dataset.theme).toBe("dark");
  });
});

describe("the shell's early script", () => {
  it("reads the key and the schemes the theme module writes", () => {
    const shell = readFileSync(new URL("../../index.html", import.meta.url), "utf8");
    expect(shell).toContain(`localStorage.getItem("${THEME_KEY}")`);
    for (const theme of THEMES.filter((theme) => theme !== "auto")) {
      expect(shell).toContain(`theme === "${theme}"`);
    }
  });
});
