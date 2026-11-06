import type { Display, Screen, ScreenDocument } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, Grid3x3 } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Link, useBlocker, useLocation, useNavigate, useParams } from "react-router";

import { client, displayModelsQuery, messageOf, queryClient, serverQuery } from "../../app/client";
import { devicesQuery } from "../../devices/queries";
import { Proof } from "../../proof/Proof";
import { screenQuery, screensQuery } from "../queries";
import { Editor, type EditorHandle, type Located } from "./Editor";
import styles from "./ScreenEditor.module.css";
import { SourcesPanel } from "./SourcesPanel";
import { STARTERS } from "./starters";
import { usePreview } from "./usePreview";
import { openingText, parseYaml, rangeOf } from "./yamlDocument";

/** Writing a screen: its YAML beside a proof the server draws at the display's
 * own pixels, with what each source answered and every problem in place. */
export function ScreenEditor() {
  const { screenId } = useParams();
  const id = screenId === undefined ? undefined : Number(screenId);
  const screen = useQuery({ ...screenQuery(id ?? 0), enabled: id !== undefined });

  if (id !== undefined && screen.isPending) return <p className={`${styles.loading} annotation`}>Loading screen</p>;
  if (id !== undefined && screen.isError) {
    return (
      <div className={styles.loading}>
        <p className="error-text">{messageOf(screen.error)}</p>
        <Link to="/screens">Back to screens</Link>
      </div>
    );
  }
  return <Workbench key={id ?? "new"} screen={screen.data} />;
}

function Workbench({ screen }: { screen: Screen | undefined }) {
  const navigate = useNavigate();
  // A screen just created keeps the text it was written as.
  const written = (useLocation().state as { text?: string } | null)?.text;
  const editor = useRef<EditorHandle>(null);
  const starter = STARTERS[0];
  const initialText = screen ? openingText(screen.document, written) : (starter?.yaml ?? "");
  const [text, setText] = useState(initialText);
  const [name, setName] = useState(screen?.name ?? starter?.name ?? "");
  const [baseline, setBaseline] = useState({ text: screen ? initialText : "", name: screen?.name ?? "" });
  const [touched, setTouched] = useState(Boolean(screen));
  const [grid, setGrid] = useState(false);

  const parsed = useMemo(() => parseYaml(text), [text]);
  const devices = useQuery(devicesQuery).data;
  const models = useQuery(displayModelsQuery).data;
  const server = useQuery(serverQuery).data;
  const secretNames = server?.secretNames ?? [];

  const [target, setTarget] = useState<string>();
  const [display, setDisplay] = useState<Display>();
  const firstDevice = devices?.[0];
  const chosenTarget = target ?? (firstDevice ? `device:${firstDevice.id}` : "model");
  const chosenDevice = chosenTarget.startsWith("device:")
    ? devices?.find((device) => `device:${device.id}` === chosenTarget)
    : undefined;
  // Without a device, the server's default display.
  const chosenDisplay: Display | undefined = chosenDevice?.display ?? display ?? server?.defaultDisplay;
  const model = models?.find((candidate) => candidate.name === chosenDisplay?.model);
  const palette = model?.palettes.find((candidate) => candidate.id === chosenDisplay?.palette);

  const preview = usePreview(parsed.value, chosenDisplay, chosenDevice?.id);

  const located: Array<Located & { path?: string | undefined }> = useMemo(
    () =>
      preview.problems.map((problem) => {
        const range = problem.path ? rangeOf(parsed.document, problem.path) : undefined;
        return { from: range?.[0] ?? 0, to: range?.[1] ?? 0, message: problem.message, path: problem.path };
      }),
    [preview.problems, parsed.document],
  );
  const inText = useMemo(() => located.filter((problem) => problem.to > problem.from), [located]);

  const valid = parsed.value !== undefined && name.trim().length > 0;

  const save = useMutation({
    mutationFn: async () => {
      const document = parsed.value as ScreenDocument;
      return screen
        ? (await client.updateScreen({ screenId: screen.id, name: name.trim(), document })).screen
        : (await client.createScreen({ name: name.trim(), document })).screen;
    },
    onSuccess: async (saved) => {
      setBaseline({ text, name: saved.name });
      queryClient.setQueryData(screenQuery(saved.id).queryKey, saved);
      await queryClient.invalidateQueries({ queryKey: screensQuery.queryKey });
      if (!screen) void navigate(`/screens/${saved.id}`, { replace: true, state: { text } });
    },
  });

  // A new screen holding an untouched starter has nothing to lose.
  const dirty = screen || save.isSuccess ? text !== baseline.text || name.trim() !== baseline.name : touched;

  // A new screen can be saved as it stands, starter and all.
  const canSave = valid && !save.isPending && (dirty || (!screen && !save.isSuccess));

  const [confirmDelete, setConfirmDelete] = useState(false);
  const remove = useMutation({
    mutationFn: () => client.deleteScreen({ screenId: screen?.id ?? 0 }),
    onSuccess: async () => {
      setBaseline({ text, name: name.trim() });
      await queryClient.invalidateQueries({ queryKey: screensQuery.queryKey });
      await queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      void navigate("/screens");
    },
  });
  const shownOn = devices?.filter((device) => device.playlist.some((entry) => entry.screenId === screen?.id)) ?? [];

  // Leaving with unsaved changes asks first, inside the app and when the tab closes.
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) =>
      dirty && !save.isPending && !remove.isPending && currentLocation.pathname !== nextLocation.pathname,
  );
  useEffect(() => {
    if (blocker.state !== "blocked") return;
    if (window.confirm("Leave this screen without saving your changes?")) blocker.proceed();
    else blocker.reset();
  }, [blocker]);
  useEffect(() => {
    if (!dirty) return;
    const warn = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [dirty]);

  // Ctrl+S or Cmd+S saves.
  const saveRef = useRef(() => {});
  saveRef.current = () => {
    if (canSave) save.mutate();
  };
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "s") {
        event.preventDefault();
        saveRef.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const status = save.isPending
    ? "Saving"
    : !screen && !save.isSuccess
      ? "Not saved yet"
      : dirty
        ? "Unsaved changes"
        : "Saved";

  return (
    <div className={styles.page}>
      <header className={styles.head}>
        <nav className={styles.trail} aria-label="Breadcrumb">
          <Link to="/screens">
            <ArrowLeft size={14} aria-hidden="true" />
            Screens
          </Link>
        </nav>
        <label className={styles.name}>
          <span className="visually-hidden">Screen name</span>
          <input
            className={styles.nameInput}
            value={name}
            maxLength={64}
            required
            placeholder="Name this screen"
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <span className={styles.status} data-dirty={dirty || undefined} role="status">
          {status}
        </span>
        <div className={styles.headActions}>
          {screen && !confirmDelete && (
            <button className="button" data-variant="danger" type="button" onClick={() => setConfirmDelete(true)}>
              Delete
            </button>
          )}
          <button
            className="button"
            data-variant="primary"
            type="button"
            disabled={!canSave}
            onClick={() => save.mutate()}
            title="Ctrl+S"
          >
            {screen ? "Save" : "Save screen"}
          </button>
        </div>
      </header>

      {confirmDelete && screen && (
        <div className={styles.confirm} role="alert">
          <p>
            Delete {screen.name}?{" "}
            {shownOn.length > 0
              ? `It leaves the playlists of ${shownOn.map((device) => device.name).join(", ")}, and those displays move on to their other screens.`
              : "No display shows it."}
          </p>
          <button className="button" data-variant="danger-solid" type="button" onClick={() => remove.mutate()} disabled={remove.isPending}>
            {remove.isPending ? "Deleting" : "Delete screen"}
          </button>
          <button className="button" data-variant="quiet" type="button" onClick={() => setConfirmDelete(false)}>
            Keep it
          </button>
          {remove.isError && <p className="error-text">{messageOf(remove.error)}</p>}
        </div>
      )}
      {save.isError && (
        <p className={`${styles.saveError} error-text`} role="alert">
          {messageOf(save.error)}
        </p>
      )}

      {!screen && !touched && (
        <section className={styles.starters} aria-labelledby="starters-title">
          <h2 id="starters-title" className="annotation">
            Start from
          </h2>
          <ul>
            {STARTERS.map((candidate) => (
              <li key={candidate.id}>
                <button
                  type="button"
                  className={styles.starter}
                  aria-pressed={text === candidate.yaml}
                  onClick={() => {
                    editor.current?.setText(candidate.yaml);
                    setName(candidate.name);
                  }}
                >
                  <strong>{candidate.name}</strong>
                  <span>{candidate.summary}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <div className={styles.bench}>
        <div className={styles.source}>
          <Editor
            ref={editor}
            initial={initialText}
            label="Screen document, YAML"
            problems={inText}
            onChange={(next) => {
              setText(next);
              if (!touched && !STARTERS.some((candidate) => candidate.yaml === next)) setTouched(true);
            }}
          />
        </div>

        <aside
          className={styles.proofColumn}
          aria-label="Proof"
          style={model ? ({ "--display-width": `${model.width}px` } as CSSProperties) : undefined}
        >
          <div className={styles.toolbar}>
            <label className={styles.target}>
              <span className="annotation">Proof on</span>
              <select className="select" value={chosenTarget} onChange={(event) => setTarget(event.target.value)}>
                {devices?.map((device) => (
                  <option key={device.id} value={`device:${device.id}`}>
                    {device.name}
                  </option>
                ))}
                <option value="model">A display model…</option>
              </select>
            </label>
            {chosenTarget === "model" && (
              <>
                <label className={styles.target}>
                  <span className="visually-hidden">Display model</span>
                  <select
                    className="select"
                    value={chosenDisplay?.model}
                    onChange={(event) => {
                      const next = models?.find((candidate) => candidate.name === event.target.value);
                      const first = next?.palettes[0];
                      if (next && first) setDisplay({ model: next.name, palette: first.id });
                    }}
                  >
                    {[...(models ?? [])]
                      .sort((a, b) => a.label.localeCompare(b.label))
                      .map((candidate) => (
                        <option key={candidate.name} value={candidate.name}>
                          {candidate.label}
                        </option>
                      ))}
                  </select>
                </label>
                <label className={styles.target}>
                  <span className="visually-hidden">Colors</span>
                  <select
                    className="select"
                    value={chosenDisplay?.palette}
                    onChange={(event) => chosenDisplay && setDisplay({ model: chosenDisplay.model, palette: event.target.value })}
                  >
                    {model?.palettes.map((candidate) => (
                      <option key={candidate.id} value={candidate.id}>
                        {candidate.name}
                      </option>
                    ))}
                  </select>
                </label>
              </>
            )}
            <button
              type="button"
              className="icon-button"
              aria-pressed={grid}
              aria-label="Layout grid"
              title="Layout grid"
              data-on={grid || undefined}
              onClick={() => setGrid((on) => !on)}
            >
              <Grid3x3 size={18} />
            </button>
            <span className={styles.drawing} aria-live="polite">
              {preview.drawing
                ? "Drawing"
                : preview.stale && preview.image
                  ? "The last proof that drew; see the problems below"
                  : ""}
            </span>
          </div>

          <Proof
            src={preview.image}
            alt="The screen as the display will show it"
            width={model?.width}
            height={model?.height}
            colors={palette?.colors}
            paletteName={palette?.name}
            empty={parsed.value === undefined ? "Fix the YAML to draw" : "Drawing"}
            busy={preview.drawing}
            overlay={grid && model ? <LayoutGrid width={model.width} height={model.height} /> : undefined}
          />

          {(preview.failure || parsed.errors.length > 0 || located.length > 0) && (
            <section className={styles.problems} aria-labelledby="problems-title">
              <h3 id="problems-title" className="annotation">
                Problems
              </h3>
              <ul>
                {parsed.errors.map((error, index) => (
                  <li key={`yaml-${index}`}>
                    <button type="button" onClick={() => editor.current?.select(error.from, error.to)}>
                      <span className={styles.where}>YAML</span> {error.message}
                    </button>
                  </li>
                ))}
                {preview.failure && <li className="error-text">{preview.failure}</li>}
                {located.map((problem, index) => (
                  <li key={index}>
                    <button
                      type="button"
                      disabled={problem.to <= problem.from}
                      onClick={() => editor.current?.select(problem.from, problem.to)}
                    >
                      {problem.path && <span className={styles.where}>{problem.path}</span>} {problem.message}
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}

          <SourcesPanel sources={preview.sources} secretNames={secretNames} />
        </aside>
      </div>
    </div>
  );
}

/** Guides over the proof: every 40 device pixels, stronger every 200, and the
 * centre lines. */
function LayoutGrid({ width, height }: { width: number; height: number }) {
  const cell = (step: number, size: number) => `${(step / size) * 100}%`;
  return (
    <div
      className={styles.grid}
      aria-hidden="true"
      style={{
        backgroundSize: `${cell(40, width)} ${cell(40, height)}, ${cell(40, width)} ${cell(40, height)}, ${cell(200, width)} ${cell(200, height)}, ${cell(200, width)} ${cell(200, height)}`,
      }}
    >
      <span className={styles.centerX} />
      <span className={styles.centerY} />
    </div>
  );
}
