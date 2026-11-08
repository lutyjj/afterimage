import type { ScheduleSlot } from "@afterimage/management-client";
import { describe, expect, it } from "vitest";

import { draftOf, entryOf, sameAs } from "./scheduleDraft";

describe("schedule drafts", () => {
  it("match the slots they were made from, equal times and all", () => {
    const saved = [
      { playlistId: 1, from: "08:00", until: "08:00" },
      { playlistId: 2, from: "00:00" },
      { playlistId: 3, days: ["monday" as const], from: "07:00", until: "09:30" },
    ];
    expect(sameAs(draftOf(saved), saved)).toBe(true);
  });

  it("save an all-day slot without its times and a timed one with them", () => {
    const [allDay, timed] = draftOf([
      { playlistId: 1, from: "08:00", until: "08:00" },
      { playlistId: 2, from: "07:00", until: "09:30" },
    ]);
    expect(allDay && entryOf(allDay)).toEqual({ playlistId: 1, from: undefined, until: undefined });
    expect(timed && entryOf(timed)).toEqual({ playlistId: 2, from: "07:00", until: "09:30" });
  });

  it("differ from the saved slots once a time changes", () => {
    const saved = [{ playlistId: 2, from: "07:00", until: "09:30" }];
    const [entry] = draftOf(saved);
    expect(entry && sameAs([{ ...entry, until: "10:00" }], saved)).toBe(false);
  });

  it("keep each row's key when they are made again from the slots they saved", () => {
    const before = draftOf<ScheduleSlot>([{ playlistId: 1 }, { playlistId: 2 }]);
    const again = draftOf<ScheduleSlot>([{ playlistId: 1 }, { playlistId: 2 }, { playlistId: 3 }], before);
    expect(again.slice(0, 2).map((entry) => entry.key)).toEqual(before.map((entry) => entry.key));
    expect(before.map((entry) => entry.key)).not.toContain(again[2]?.key);
  });
});
