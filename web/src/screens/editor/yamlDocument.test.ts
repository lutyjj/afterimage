import { describe, expect, it } from "vitest";

import { openingText, parseYaml, rangeOf, toYaml } from "./yamlDocument";

const text = `content:
  column:
    children:
      - title:
          text: Hello
      - value:
          text: "{{ 1 + }}"
`;

describe("problems located in the YAML", () => {
  it("finds the node a JSON Pointer names", () => {
    const { document } = parseYaml(text);
    const range = rangeOf(document, "/content/column/children/1/value/text");
    expect(range && text.slice(...range).trim()).toBe('"{{ 1 + }}"');
  });

  it("falls back to the nearest node that exists", () => {
    const { document } = parseYaml(text);
    const range = rangeOf(document, "/content/column/children/0/title/size");
    expect(range && text.slice(...range)).toContain("text: Hello");
  });

  it("reports YAML that does not parse at its place", () => {
    const parsed = parseYaml("content:\n  title: [unclosed\n");
    expect(parsed.value).toBeUndefined();
    expect(parsed.errors.length).toBeGreaterThan(0);
  });
});

describe("a stored document as YAML", () => {
  it("leaves out the defaults the server filled in", () => {
    const stored = {
      sources: { weather: { url: "https://weather.example.test/now", method: "GET" } },
      content: {
        column: {
          gap: "medium",
          align: "start",
          justify: "start",
          grow: false,
          children: [{ value: { text: "7°", size: "xlarge", fit: false, tabular: false } }],
        },
      },
    };
    expect(toYaml(stored)).toBe(`sources:
  weather:
    url: https://weather.example.test/now
content:
  column:
    children:
      - value:
          text: 7°
          size: xlarge
`);
  });
});

describe("openingText", () => {
  // As the server stores it: every default filled in.
  const stored = (text: string) => ({
    content: {
      column: {
        gap: "medium",
        align: "start",
        justify: "start",
        grow: false,
        children: [{ title: { text, size: "base" } }],
      },
    },
  });
  const written = `# The greeting.
content:
  column:
    children:
      - title:
          text: Hello
`;

  it("keeps the text a screen was written as while it says what is stored", () => {
    expect(openingText(stored("Hello"), written)).toBe(written);
  });

  it("shows what is stored once a later save changed it", () => {
    expect(openingText(stored("Goodbye"), written)).toBe(toYaml(stored("Goodbye")));
  });

  it("shows what is stored when nothing was written here", () => {
    expect(openingText(stored("Hello"), undefined)).toBe(toYaml(stored("Hello")));
  });
});
