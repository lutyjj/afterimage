import { Button, classes } from "linen";

import { SAVE_STATES, type SaveState } from "./saveState";

/** Every word the button says, laid in the same cell so the button keeps the
 * width of the longest and its neighbour never shifts. */
const SAVE_LABELS = [...new Set(Object.values(SAVE_STATES).map((state) => state.label))];

/** An editor's one primary act, which carries its state: in ink while there is
 * something to save, quiet once saved. The state is read out too, for a reader
 * who does not see the button change. */
export function SaveButton({ state, disabled, onSave }: { state: SaveState; disabled: boolean; onSave: () => void }) {
  const shown = SAVE_STATES[state];
  return (
    <>
      <span role="status" className="sr-only">
        {shown.status}
      </span>
      <Button variant={shown.quiet ? "default" : "primary"} disabled={disabled} onClick={onSave} title="Ctrl+S">
        <span className="grid">
          {SAVE_LABELS.map((label) => (
            <span key={label} className={classes("col-start-1 row-start-1", label !== shown.label && "invisible")}>
              {label}
            </span>
          ))}
        </span>
      </Button>
    </>
  );
}
