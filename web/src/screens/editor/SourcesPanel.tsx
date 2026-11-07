import type { SourceResult } from "@afterimage/management-client";
import { ChevronRight } from "lucide-react";
import { classes } from "linen";
import { useState } from "react";

/** What each data source answered, as a tree whose leaves copy the template that
 * reads them. */
export function SourcesPanel({
  sources,
  secretNames,
}: {
  sources: Record<string, SourceResult> | undefined;
  secretNames: string[];
}) {
  // The clipboard needs a secure context; over plain HTTP, or when the browser
  // refuses, the expression is shown to copy by hand.
  const [copied, setCopied] = useState<{ expression: string; done: boolean }>();
  const copy = (expression: string) => {
    const manual = () => setCopied({ expression, done: false });
    if (!navigator.clipboard) return manual();
    navigator.clipboard.writeText(expression).then(() => setCopied({ expression, done: true }), manual);
  };
  const entries = Object.entries(sources ?? {});

  return (
    <section className="grid min-w-0 gap-2" aria-labelledby="sources-title">
      <h3 id="sources-title" className="text-sm font-medium text-text-secondary">
        Data sources
      </h3>
      {entries.length === 0 && (
        <p className="text-sm text-text-secondary">
          This screen reads no data. Add <code className="font-mono text-xs">sources:</code> with a name and a{" "}
          <code className="font-mono text-xs">url</code>; its JSON appears here.
        </p>
      )}
      {entries.map(([name, result]) => (
        <details key={name} className="group/source border-t border-border" open={entries.length === 1}>
          <summary className="flex cursor-pointer list-none items-center gap-2 py-2 text-sm [&::-webkit-details-marker]:hidden">
            <ChevronRight
              aria-hidden="true"
              className="size-3.5 [transition:rotate_var(--motion-state)] group-open/source:rotate-90"
            />
            <code className="font-mono text-xs">{name}</code>
            {result.error !== undefined && <span className="text-xs font-semibold text-danger">no data</span>}
          </summary>
          {result.error !== undefined ? (
            <p className="pb-2 text-sm text-danger">{result.error}</p>
          ) : (
            <div className="mb-2 max-h-88 overflow-auto rounded-button border border-border bg-surface px-3 py-2 font-mono text-xs/[1.6]">
              <Node value={result.data} path={name} depth={0} onCopy={copy} />
            </div>
          )}
        </details>
      ))}
      {copied && (
        <p role="status" className="text-xs">
          {copied.done ? "Copied" : "Copy"}{" "}
          <code className={classes("font-mono", !copied.done && "select-all")}>{copied.expression}</code>
        </p>
      )}
      {secretNames.length > 0 && (
        <div className="grid gap-1 border-t border-border pt-2">
          <span className="text-sm font-medium text-text-secondary">Secrets</span>
          <p className="text-sm text-text-secondary">
            Data source requests read them; screens never show them.{" "}
            {secretNames.map((name) => (
              <button
                key={name}
                type="button"
                className={classes(LEAF_CLASS, "font-mono text-xs text-text")}
                onClick={() => copy(`{{ secrets.${name} }}`)}
              >
                secrets.{name}
              </button>
            ))}
          </p>
        </div>
      )}
    </section>
  );
}

/** A value or a branch that copies the template reading it. */
const LEAF_CLASS = "cursor-copy rounded-input px-1 text-left [overflow-wrap:anywhere] hover:bg-accent-weak";

const IDENTIFIER = /^[A-Za-z_][A-Za-z0-9_]*$/;

function child(path: string, key: string | number) {
  if (typeof key === "number") return `${path}[${key}]`;
  return IDENTIFIER.test(key) ? `${path}.${key}` : `${path}[${JSON.stringify(key)}]`;
}

const MAX_CHILDREN = 50;

function Node({
  value,
  path,
  depth,
  onCopy,
}: {
  value: unknown;
  path: string;
  depth: number;
  onCopy: (expression: string) => void;
}) {
  if (value === null || typeof value !== "object") {
    return (
      <button type="button" className={LEAF_CLASS} onClick={() => onCopy(`{{ ${path} }}`)} title={`Copy {{ ${path} }}`}>
        <span className={typeof value === "string" ? "text-text" : "text-text-secondary"}>{JSON.stringify(value)}</span>
      </button>
    );
  }
  const items: Array<[string | number, unknown]> = Array.isArray(value)
    ? value.map((item, index) => [index, item])
    : Object.entries(value);
  const shown = items.slice(0, MAX_CHILDREN);
  return (
    <details className="group/branch basis-full" open={depth < 2}>
      <summary className="flex cursor-pointer list-none items-center gap-0.5 [&::-webkit-details-marker]:hidden">
        <ChevronRight
          aria-hidden="true"
          className="size-3 shrink-0 [transition:rotate_var(--motion-state)] group-open/branch:rotate-90"
        />
        <button
          type="button"
          className={classes(LEAF_CLASS, "text-text-secondary")}
          onClick={(event) => {
            // Copies without opening or closing the branch.
            event.preventDefault();
            onCopy(`{{ ${path} }}`);
          }}
          title={`Copy {{ ${path} }}`}
        >
          {Array.isArray(value) ? `list of ${items.length}` : `${items.length} members`}
        </button>
      </summary>
      <ul className="ml-2 border-l border-border pl-3">
        {shown.map(([key, item]) => (
          <li key={key} className="flex flex-wrap items-baseline gap-x-2">
            <span className="text-text after:content-[':']">{typeof key === "number" ? `[${key}]` : key}</span>
            <Node value={item} path={child(path, key)} depth={depth + 1} onCopy={onCopy} />
          </li>
        ))}
        {items.length > shown.length && <li className="text-text-secondary">and {items.length - shown.length} more</li>}
      </ul>
    </details>
  );
}
