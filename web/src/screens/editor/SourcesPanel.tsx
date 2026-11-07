import type { SourceResult } from "@afterimage/management-client";
import { ChevronRight } from "lucide-react";
import { useState } from "react";

import styles from "./SourcesPanel.module.css";

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
    <section className={styles.panel} aria-labelledby="sources-title">
      <h3 id="sources-title" className={styles.title}>
        Data sources
      </h3>
      {entries.length === 0 && (
        <p className="hint">
          This screen reads no data. Add <code>sources:</code> with a name and a <code>url</code>; its JSON
          appears here.
        </p>
      )}
      {entries.map(([name, result]) => (
        <details key={name} className={styles.source} open={entries.length === 1}>
          <summary>
            <ChevronRight size={14} className={styles.chevron} aria-hidden="true" />
            <code>{name}</code>
            {result.error !== undefined && <span className={styles.failed}>no data</span>}
          </summary>
          {result.error !== undefined ? (
            <p className="error-text">{result.error}</p>
          ) : (
            <div className={styles.tree}>
              <Node value={result.data} path={name} depth={0} onCopy={copy} />
            </div>
          )}
        </details>
      ))}
      {copied && (
        <p className={styles.copied} role="status">
          {copied.done ? "Copied" : "Copy"} <code data-select={!copied.done || undefined}>{copied.expression}</code>
        </p>
      )}
      {secretNames.length > 0 && (
        <div className={styles.secrets}>
          <span className={styles.title}>Secrets</span>
          <p className="hint">
            Data source requests read them; screens never show them.{" "}
            {secretNames.map((name) => (
              <button key={name} type="button" className={styles.leaf} onClick={() => copy(`{{ secrets.${name} }}`)}>
                secrets.{name}
              </button>
            ))}
          </p>
        </div>
      )}
    </section>
  );
}

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
      <button type="button" className={styles.leaf} onClick={() => onCopy(`{{ ${path} }}`)} title={`Copy {{ ${path} }}`}>
        <span className={styles.value} data-type={value === null ? "null" : typeof value}>
          {JSON.stringify(value)}
        </span>
      </button>
    );
  }
  const items: Array<[string | number, unknown]> = Array.isArray(value)
    ? value.map((item, index) => [index, item])
    : Object.entries(value);
  const shown = items.slice(0, MAX_CHILDREN);
  return (
    <details className={styles.branch} open={depth < 2}>
      <summary>
        <ChevronRight size={12} className={styles.chevron} aria-hidden="true" />
        <button
          type="button"
          className={styles.copyBranch}
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
      <ul>
        {shown.map(([key, item]) => (
          <li key={key}>
            <span className={styles.key}>{typeof key === "number" ? `[${key}]` : key}</span>
            <Node value={item} path={child(path, key)} depth={depth + 1} onCopy={onCopy} />
          </li>
        ))}
        {items.length > shown.length && <li className="hint">and {items.length - shown.length} more</li>}
      </ul>
    </details>
  );
}
