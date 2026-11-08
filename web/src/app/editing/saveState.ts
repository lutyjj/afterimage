/** Where an editor's work stands against what the server holds. */
export type SaveState = "new" | "unsaved" | "saving" | "saved";

export function saveState({
  saving,
  stored,
  dirty,
}: {
  saving: boolean;
  /** The server holds it: it was opened, or saved once already. */
  stored: boolean;
  dirty: boolean;
}): SaveState {
  if (saving) return "saving";
  if (!stored) return "new";
  return dirty ? "unsaved" : "saved";
}

/** What each state reads out, what the save button says, and whether the
 * button is quiet, as it is once nothing is left to save. */
export const SAVE_STATES: Record<SaveState, { status: string; label: string; quiet: boolean }> = {
  new: { status: "Not saved yet", label: "Save", quiet: false },
  unsaved: { status: "Unsaved changes", label: "Save", quiet: false },
  saving: { status: "Saving", label: "Saving", quiet: false },
  saved: { status: "Saved", label: "Saved", quiet: true },
};
