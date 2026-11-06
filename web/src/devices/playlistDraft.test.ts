import { describe, expect, it } from "vitest";

import { draftOf, entryOf, sameAs } from "./playlistDraft";

describe("playlist drafts", () => {
  it("match the playlist they were made from, equal times and all", () => {
    const saved = [
      { screenId: 1, from: "08:00", until: "08:00" },
      { screenId: 2, from: "00:00" },
      { screenId: 3, days: ["monday" as const], from: "07:00", until: "09:30" },
    ];
    expect(sameAs(draftOf(saved), saved)).toBe(true);
  });

  it("save an all-day entry without its times and a timed one with them", () => {
    const [allDay, timed] = draftOf([
      { screenId: 1, from: "08:00", until: "08:00" },
      { screenId: 2, from: "07:00", until: "09:30" },
    ]);
    expect(allDay && entryOf(allDay)).toEqual({ screenId: 1, from: undefined, until: undefined });
    expect(timed && entryOf(timed)).toEqual({ screenId: 2, from: "07:00", until: "09:30" });
  });

  it("differ from the saved playlist once a time changes", () => {
    const saved = [{ screenId: 2, from: "07:00", until: "09:30" }];
    const [entry] = draftOf(saved);
    expect(entry && sameAs([{ ...entry, until: "10:00" }], saved)).toBe(false);
  });
});
