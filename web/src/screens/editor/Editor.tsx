import { HighlightStyle, indentUnit, syntaxHighlighting } from "@codemirror/language";
import { forceLinting, linter, type Diagnostic } from "@codemirror/lint";
import { EditorSelection, StateEffect, StateField } from "@codemirror/state";
import { tags } from "@lezer/highlight";
import { basicSetup, EditorView } from "codemirror";
import { yamlSchema } from "codemirror-json-schema/yaml";
import { wrappedLineIndent } from "codemirror-wrapped-line-indent";
import { useEffect, useRef, type Ref } from "react";

import { documentSchema } from "./schema";
import { parseYaml } from "./yamlDocument";
import styles from "./Editor.module.css";

/** Problems the server found, placed in the text they were found in. */
export interface Located {
  from: number;
  to: number;
  message: string;
}

const setServerProblems = StateEffect.define<Located[]>();

/** The server's problems, moved along as the text changes after they arrived. */
const serverProblems = StateField.define<Located[]>({
  create: () => [],
  update(problems, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setServerProblems)) return effect.value;
    }
    if (!transaction.docChanged) return problems;
    return problems.map((problem) => ({
      ...problem,
      from: transaction.changes.mapPos(problem.from),
      to: transaction.changes.mapPos(problem.to, 1),
    }));
  },
});

const serverLinter = linter(
  (view) => {
    const length = view.state.doc.length;
    return view.state.field(serverProblems).map(
      (problem): Diagnostic => ({
        from: Math.min(problem.from, length),
        to: Math.min(Math.max(problem.to, problem.from), length),
        severity: "error",
        source: "Preview",
        message: problem.message,
      }),
    );
  },
  {
    delay: 0,
    needsRefresh: (update) => update.transactions.some((tr) => tr.effects.some((effect) => effect.is(setServerProblems))),
  },
);

const yamlLinter = linter((view) =>
  parseYaml(view.state.doc.toString()).errors.map(
    (error): Diagnostic => ({ ...error, severity: "error", source: "YAML" }),
  ),
);

/** Keys in weight, so a document's structure reads at a glance; what it says in
 * the secondary text; numbers, literals and punctuation quieter. The ink stays
 * for what a hand acts on. */
const highlight = HighlightStyle.define([
  { tag: [tags.propertyName, tags.definition(tags.propertyName)], color: "var(--text)", fontWeight: "600" },
  { tag: [tags.string, tags.content], color: "var(--text-secondary)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom], color: "var(--text-secondary)" },
  { tag: [tags.comment, tags.meta], color: "var(--text-muted)", fontStyle: "italic" },
  { tag: [tags.punctuation, tags.separator, tags.squareBracket, tags.brace], color: "var(--text-muted)" },
]);

const theme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "var(--surface)", color: "var(--text)", fontSize: "13px" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.65" },
  ".cm-content": { caretColor: "var(--ink)", padding: "12px 0" },
  ".cm-cursor": { borderLeftColor: "var(--ink)", borderLeftWidth: "2px" },
  ".cm-gutters": { backgroundColor: "var(--surface)", color: "var(--text-muted)", borderRight: "1px solid var(--border)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--text)" },
  ".cm-activeLine": { backgroundColor: "color-mix(in oklch, var(--ink-weak) 55%, transparent)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in oklch, var(--ink) 22%, transparent)",
  },
  ".cm-tooltip": {
    border: "1px solid var(--border)",
    backgroundColor: "var(--surface)",
    borderRadius: "var(--radius-control)",
    boxShadow: "var(--shadow-overlay)",
  },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--ink)", color: "var(--on-ink)" },
  ".cm-tooltip.cm-tooltip-hover": { maxWidth: "28rem", padding: "6px 10px", fontFamily: "var(--font)" },
  ".cm-diagnostic-error": { borderLeftColor: "var(--danger)" },
  ".cm-lintRange-error": {
    backgroundImage: "none",
    textDecoration: "underline wavy var(--danger)",
    textUnderlineOffset: "3px",
  },
  ".cm-panels": { backgroundColor: "var(--sunken)", color: "var(--text)" },
  ".cm-foldPlaceholder": { backgroundColor: "var(--ink-weak)", border: "none", color: "var(--ink)" },
});

export interface EditorHandle {
  /** Selects a range and brings it into view. */
  select(from: number, to: number): void;
  setText(text: string): void;
}

export interface EditorProps {
  initial: string;
  onChange: (text: string) => void;
  problems: Located[];
  label: string;
  ref?: Ref<EditorHandle>;
}

/** The YAML of a screen document, with completion, hover help and lint from the
 * model's schema, and the server's problems where they arose. */
export function Editor({ initial, onChange, problems, label, ref }: EditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const changed = useRef(onChange);
  changed.current = onChange;

  useEffect(() => {
    if (!host.current) return;
    const editor = new EditorView({
      parent: host.current,
      doc: initial,
      extensions: [
        basicSetup,
        indentUnit.of("  "),
        // Long templates wrap under their own indentation, so the pane beside the
        // preview never hides the end of a line.
        EditorView.lineWrapping,
        wrappedLineIndent,
        yamlSchema(documentSchema as Parameters<typeof yamlSchema>[0]),
        yamlLinter,
        serverProblems,
        serverLinter,
        theme,
        syntaxHighlighting(highlight),
        EditorView.contentAttributes.of({ "aria-label": label }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) changed.current(update.state.doc.toString());
        }),
      ],
    });
    view.current = editor;
    return () => {
      editor.destroy();
      view.current = null;
    };
    // The editor is created once; `initial` seeds it and `setText` replaces it.
  }, []);

  useEffect(() => {
    const editor = view.current;
    if (!editor) return;
    editor.dispatch({ effects: setServerProblems.of(problems) });
    forceLinting(editor);
  }, [problems]);

  useEffect(() => {
    if (!ref) return;
    const handle: EditorHandle = {
      select(from, to) {
        const editor = view.current;
        if (!editor) return;
        const length = editor.state.doc.length;
        editor.dispatch({
          selection: EditorSelection.range(Math.min(from, length), Math.min(to, length)),
          scrollIntoView: true,
        });
        editor.focus();
      },
      setText(text) {
        const editor = view.current;
        if (!editor) return;
        editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: text } });
      },
    };
    if (typeof ref === "function") {
      ref(handle);
    } else {
      ref.current = handle;
    }
  }, [ref]);

  return <div ref={host} className={styles.editor} />;
}
