import { describe, expect, it } from "vitest";

import { SAVE_STATES, saveState } from "./saveState";

const shown = (inputs: Parameters<typeof saveState>[0]) => SAVE_STATES[saveState(inputs)];

describe("the editor's save state", () => {
  it("is a quiet Saved for a screen opened and left as it was", () => {
    expect(shown({ saving: false, stored: true, dirty: false })).toEqual({
      status: "Saved",
      label: "Saved",
      quiet: true,
    });
  });

  it("asks to be saved, in ink, once the screen is changed", () => {
    expect(shown({ saving: false, stored: true, dirty: true })).toEqual({
      status: "Unsaved changes",
      label: "Save",
      quiet: false,
    });
  });

  it("asks to be saved for a new screen, changed or not", () => {
    for (const dirty of [false, true]) {
      expect(shown({ saving: false, stored: false, dirty })).toEqual({
        status: "Not saved yet",
        label: "Save",
        quiet: false,
      });
    }
  });

  it("says it is saving while a save runs, whatever came before", () => {
    for (const stored of [false, true]) {
      expect(saveState({ saving: true, stored, dirty: true })).toBe("saving");
    }
  });
});
