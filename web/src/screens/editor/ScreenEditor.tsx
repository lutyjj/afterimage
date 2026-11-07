import type { Display, Screen, ScreenDocument } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  Button,
  buttonClass,
  CellSelect,
  classes,
  ConfirmDialog,
  Content,
  EmptyState,
  IconButton,
  TitleField,
  TopBar,
  useToast,
  WIDE_COLUMN_CLASS,
} from "linen";
import { Grid3x3, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Link, useBlocker, useLocation, useNavigate, useParams } from "react-router";

import { client, displayModelsQuery, messageOf, queryClient, serverQuery } from "../../app/client";
import { devicesQuery } from "../../devices/queries";
import { Panel } from "../../panel/Panel";
import { screenQuery, screensQuery } from "../queries";
import { Editor, type EditorHandle, type Located } from "./Editor";
import { SourcesPanel } from "./SourcesPanel";
import { STARTERS } from "./starters";
import { usePreview } from "./usePreview";
import { openingText, parseYaml, rangeOf } from "./yamlDocument";

const TRAIL = [{ to: "/screens", label: "Screens" }];

const SAVE_LABELS = { save: "Save", saving: "Saving", saved: "Saved" } as const;

/** The save button's words, every one laid in the same cell so the button
 * keeps the width of the longest and its neighbour never shifts. */
function SaveLabel({ shown }: { shown: keyof typeof SAVE_LABELS }) {
  return (
    <span className="grid">
      {Object.entries(SAVE_LABELS).map(([key, text]) => (
        <span key={key} className={classes("col-start-1 row-start-1", key !== shown && "invisible")}>
          {text}
        </span>
      ))}
    </span>
  );
}

/** Writing a screen: its YAML beside a preview the server draws at the display's
 * own pixels, with what each source answered and every problem in place. */
export function ScreenEditor() {
  const { screenId } = useParams();
  const id = screenId === undefined ? undefined : Number(screenId);
  const screen = useQuery({ ...screenQuery(id ?? 0), enabled: id !== undefined });

  if (id !== undefined && screen.isPending) return <TopBar trail={TRAIL} title="Screen" wide />;
  if (id !== undefined && screen.isError) {
    return (
      <>
        <TopBar trail={TRAIL} title="Screen" wide />
        <Content>
          <EmptyState title="This screen could not load" detail={messageOf(screen.error)}>
            <Link to="/screens" className={buttonClass()}>
              Back to screens
            </Link>
          </EmptyState>
        </Content>
      </>
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

  const toast = useToast();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const remove = useMutation({
    mutationFn: () => client.deleteScreen({ screenId: screen?.id ?? 0 }),
    onSuccess: async () => {
      setBaseline({ text, name: name.trim() });
      await queryClient.invalidateQueries({ queryKey: screensQuery.queryKey });
      await queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      toast.show({ text: `${screen?.name ?? "The screen"} deleted` });
      void navigate("/screens");
    },
    onError: (error) => toast.show({ text: `The screen was not deleted. ${messageOf(error)}`, sticky: true }),
  });
  const shownOn = devices?.filter((device) => device.playlist.some((entry) => entry.screenId === screen?.id)) ?? [];

  // Leaving with unsaved changes asks first, inside the app and when the tab closes.
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) =>
      dirty && !save.isPending && !remove.isPending && currentLocation.pathname !== nextLocation.pathname,
  );
  // Set as the person chooses to leave, so the dialog closing does not stay.
  const leaving = useRef(false);
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
  const saved = status === "Saved";

  return (
    <div
      className="flex flex-col lg:h-dvh"
      style={model ? ({ "--display-width": `${model.width}px` } as CSSProperties) : undefined}
    >
      <TopBar
        trail={TRAIL}
        wide
        title={
          <TitleField
            aria-label="Screen name"
            value={name}
            maxLength={64}
            required
            placeholder="Name this screen"
            onChange={(event) => setName(event.target.value)}
          />
        }
        meta={
          // The eye reads the state from the save button: quiet once saved,
          // in ink while there is something to save.
          <span role="status" className="sr-only">
            {status}
          </span>
        }
      >
        {screen && (
          <IconButton
            variant="destructive"
            aria-label="Delete screen"
            title="Delete screen"
            disabled={remove.isPending}
            onClick={() => setConfirmDelete(true)}
          >
            <Trash2 aria-hidden="true" className="size-4" />
          </IconButton>
        )}
        <Button
          variant={saved ? "default" : "primary"}
          disabled={!canSave}
          onClick={() => save.mutate()}
          title="Ctrl+S"
        >
          <SaveLabel shown={save.isPending ? "saving" : saved ? "saved" : "save"} />
        </Button>
      </TopBar>

      <div className={classes(WIDE_COLUMN_CLASS, "flex min-h-0 flex-1 flex-col gap-3 py-3")}>
        {save.isError && (
          <p role="alert" className="max-w-3xl text-sm text-danger">
            {messageOf(save.error)}
          </p>
        )}

        {!screen && !touched && (
          <section aria-labelledby="starters-title" className="grid gap-2">
            <h2 id="starters-title" className="text-sm font-medium text-text-secondary">
              Start from an example
            </h2>
            <ul className="flex gap-2 overflow-x-auto pb-1">
              {STARTERS.map((candidate) => {
                const chosen = text === candidate.yaml;
                return (
                  <li key={candidate.id} className="shrink-0">
                    <button
                      type="button"
                      aria-pressed={chosen}
                      onClick={() => {
                        editor.current?.setText(candidate.yaml);
                        setName(candidate.name);
                      }}
                      className={classes(
                        "grid h-full w-60 cursor-pointer content-start gap-0.5 rounded-button border px-3 py-2 text-left",
                        chosen
                          ? "border-accent bg-accent-weak"
                          : "border-border-strong bg-surface hover:border-accent hover:bg-accent-weak",
                      )}
                    >
                      <span className={classes("text-sm font-medium", chosen && "text-accent")}>{candidate.name}</span>
                      <span className="text-xs text-text-secondary">{candidate.summary}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </section>
        )}

        <div className="grid min-h-0 flex-1 gap-5 lg:grid-cols-[minmax(24rem,1fr)_minmax(0,calc(var(--display-width,50%)+0.5rem))]">
          <div className="min-h-0 min-w-0 max-lg:h-[60dvh]">
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
            aria-label="Preview"
            className="grid min-h-0 min-w-0 content-start gap-4 max-lg:w-[min(100%,var(--display-width,100%))] max-lg:justify-self-center lg:overflow-y-auto lg:pr-2"
          >
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm text-text-secondary">Preview on</span>
              <CellSelect
                aria-label="Preview on"
                className="max-w-64"
                choices={[
                  ...(devices ?? []).map((device) => ({ value: `device:${device.id}`, label: device.name })),
                  { value: "model", label: "A display model" },
                ]}
                value={chosenTarget}
                onValueChange={setTarget}
              />
              {chosenTarget === "model" && (
                <>
                  <CellSelect
                    aria-label="Display model"
                    className="max-w-64"
                    choices={[...(models ?? [])]
                      .sort((a, b) => a.label.localeCompare(b.label))
                      .map((candidate) => ({ value: candidate.name, label: candidate.label }))}
                    value={chosenDisplay?.model ?? ""}
                    onValueChange={(value) => {
                      const next = models?.find((candidate) => candidate.name === value);
                      const first = next?.palettes[0];
                      if (next && first) setDisplay({ model: next.name, palette: first.id });
                    }}
                  />
                  <CellSelect
                    aria-label="Colors"
                    className="max-w-64"
                    choices={(model?.palettes ?? []).map((candidate) => ({
                      value: candidate.id,
                      label: candidate.name,
                    }))}
                    value={chosenDisplay?.palette ?? ""}
                    onValueChange={(value) =>
                      chosenDisplay && setDisplay({ model: chosenDisplay.model, palette: value })
                    }
                  />
                </>
              )}
              <IconButton
                aria-label="Layout grid"
                aria-pressed={grid}
                title="Layout grid"
                className={classes(
                  grid && "border-accent bg-accent-weak text-accent hover:border-accent hover:bg-accent-weak",
                )}
                onClick={() => setGrid((on) => !on)}
              >
                <Grid3x3 aria-hidden="true" className="size-4" />
              </IconButton>
              <span aria-live="polite" className="text-sm text-text-muted">
                {preview.drawing
                  ? "Drawing"
                  : preview.stale && preview.image
                    ? "The last preview that drew; see the problems below"
                    : ""}
              </span>
            </div>

            <Panel
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
              <section aria-labelledby="problems-title" className="grid gap-2">
                <h3 id="problems-title" className="text-sm font-semibold text-danger">
                  Problems
                </h3>
                <ul className="overflow-hidden rounded-button border border-[color-mix(in_oklab,var(--color-danger)_30%,var(--color-border))] bg-surface text-sm">
                  {parsed.errors.map((error, index) => (
                    <li key={`yaml-${index}`} className="border-t border-border first:border-t-0">
                      <button
                        type="button"
                        className="w-full cursor-pointer px-3 py-2 text-left hover:bg-danger/6"
                        onClick={() => editor.current?.select(error.from, error.to)}
                      >
                        <Where>YAML</Where> {error.message}
                      </button>
                    </li>
                  ))}
                  {preview.failure && (
                    <li className="border-t border-border px-3 py-2 text-danger first:border-t-0">{preview.failure}</li>
                  )}
                  {located.map((problem, index) => (
                    <li key={index} className="border-t border-border first:border-t-0">
                      <button
                        type="button"
                        disabled={problem.to <= problem.from}
                        className="w-full cursor-pointer px-3 py-2 text-left enabled:hover:bg-danger/6 disabled:cursor-default"
                        onClick={() => editor.current?.select(problem.from, problem.to)}
                      >
                        {problem.path && <Where>{problem.path}</Where>} {problem.message}
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

      {screen && (
        <ConfirmDialog
          open={confirmDelete}
          onOpenChange={setConfirmDelete}
          title={`Delete ${screen.name}?`}
          detail={
            shownOn.length > 0
              ? `It leaves the playlists of ${shownOn.map((device) => device.name).join(", ")}, and those displays move on to their other screens.`
              : "No display shows it."
          }
          confirmLabel="Delete screen"
          destructive
          onConfirm={() => remove.mutate()}
        />
      )}
      <ConfirmDialog
        open={blocker.state === "blocked"}
        onOpenChange={(open) => {
          if (open || leaving.current) return;
          blocker.reset?.();
        }}
        title="Leave without saving?"
        detail="The changes to this screen since it was last saved are lost."
        confirmLabel="Leave"
        destructive
        onConfirm={() => {
          leaving.current = true;
          blocker.proceed?.();
        }}
      />
    </div>
  );
}

/** Where in the document a problem arose. */
function Where({ children }: { children: string }) {
  return <span className="mr-1 font-mono text-xs text-danger">{children}</span>;
}

/** Guides over the preview: every 40 device pixels, stronger every 200, and the
 * centre lines. */
function LayoutGrid({ width, height }: { width: number; height: number }) {
  const cell = (step: number, size: number) => `${(step / size) * 100}%`;
  return (
    <div
      aria-hidden="true"
      className="layout-grid pointer-events-none absolute inset-0"
      style={{
        backgroundSize: `${cell(40, width)} ${cell(40, height)}, ${cell(40, width)} ${cell(40, height)}, ${cell(200, width)} ${cell(200, height)}, ${cell(200, width)} ${cell(200, height)}`,
      }}
    >
      <span data-axis="x" className="layout-centre absolute inset-y-0 left-1/2 w-px" />
      <span data-axis="y" className="layout-centre absolute inset-x-0 top-1/2 h-px" />
    </div>
  );
}
