import { parseDocument, stringify, type Document } from "yaml";

import { withoutDefaults } from "./schema";

export interface Parsed {
  document: Document;
  /** The YAML as a value; absent when the YAML does not parse. */
  value?: unknown;
  errors: Array<{ from: number; to: number; message: string }>;
}

export function parseYaml(text: string): Parsed {
  const document = parseDocument(text, { prettyErrors: false, uniqueKeys: true });
  const errors = document.errors.map((error) => ({
    from: error.pos[0],
    to: Math.max(error.pos[1], error.pos[0] + 1),
    message: error.message.split("\n")[0] ?? error.message,
  }));
  return errors.length > 0 ? { document, errors } : { document, value: document.toJS(), errors };
}

/** The order a document reads in: where its data comes from, its title bar,
 * then what it shows. */
const ORDER = ["sources", "titleBar", "content"];

/** A stored document as YAML, without the defaults the server filled in. */
export function toYaml(value: unknown): string {
  const document = withoutDefaults(value) as Record<string, unknown>;
  const ordered = Object.fromEntries(
    Object.entries(document).sort(([a], [b]) => ORDER.indexOf(a) - ORDER.indexOf(b)),
  );
  return stringify(ordered, { lineWidth: 0, singleQuote: false });
}

/** A value as JSON with every object's keys in order, so two documents compare
 * by what they say. */
function canonical(value: unknown): string {
  return JSON.stringify(value, (_key, item: unknown) =>
    item && typeof item === "object" && !Array.isArray(item)
      ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b)))
      : item,
  );
}

/** The text a stored document opens with: the text it was written as, comments
 * and all, while that text still says what is stored; otherwise the stored
 * document as YAML, so a save made since is never shown as an older text. */
export function openingText(stored: unknown, written: string | undefined): string {
  if (written !== undefined) {
    const value = parseYaml(written).value;
    if (value !== undefined && canonical(withoutDefaults(value)) === canonical(withoutDefaults(stored))) {
      return written;
    }
  }
  return toYaml(stored);
}

/** Where in the text a JSON Pointer points: the node it names, or the nearest
 * node above it that exists. */
export function rangeOf(document: Document, pointer: string): [number, number] | undefined {
  const segments = pointer
    .split("/")
    .slice(1)
    .map((segment) => segment.replace(/~1/g, "/").replace(/~0/g, "~"))
    .map((segment) => (/^\d+$/.test(segment) ? Number(segment) : segment));
  for (let length = segments.length; length >= 0; length--) {
    const node = length === 0 ? document.contents : document.getIn(segments.slice(0, length), true);
    if (node && typeof node === "object" && "range" in node && Array.isArray(node.range)) {
      const [from, , end] = node.range as [number, number, number];
      return [from, Math.max(end, from + 1)];
    }
  }
  return undefined;
}
