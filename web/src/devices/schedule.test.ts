import { describe, expect, it } from "vitest";

import { dayOf, isAsleep, isCurrent, nowIn, overlapOf, playlistAt, spansOn } from "./schedule";

// Cases from the server's own rule (crates/server/src/schedule.rs).
describe("the rule for schedule slots and sleep windows", () => {
  it("makes hours without days or times current all day, every day", () => {
    expect(spansOn({}, "wednesday")).toEqual([[0, 1440]]);
  });

  it("treats times as a half-open range of the day", () => {
    const morning = { days: ["wednesday" as const], from: "07:00", until: "09:30" };
    expect(isCurrent(morning, "wednesday", 6 * 60 + 59)).toBe(false);
    expect(isCurrent(morning, "wednesday", 7 * 60)).toBe(true);
    expect(isCurrent(morning, "wednesday", 9 * 60 + 29)).toBe(true);
    expect(isCurrent(morning, "wednesday", 9 * 60 + 30)).toBe(false);
    expect(isCurrent(morning, "thursday", 8 * 60)).toBe(false);
  });

  it("runs hours past midnight on the day they start", () => {
    const night = { days: ["tuesday" as const], from: "22:00", until: "02:00" };
    expect(spansOn(night, "tuesday")).toEqual([[22 * 60, 1440]]);
    expect(spansOn(night, "wednesday")).toEqual([[0, 2 * 60]]);
    expect(spansOn(night, "monday")).toEqual([]);
  });

  it("makes equal times mean the whole of each of its days", () => {
    const allDay = { days: ["sunday" as const], from: "06:00", until: "06:00" };
    expect(spansOn(allDay, "sunday")).toEqual([[0, 1440]]);
    expect(spansOn(allDay, "monday")).toEqual([]);
  });
});

describe("now in the server's time zone", () => {
  it("reads the weekday and minute in that zone, not the browser's", () => {
    // 2026-11-04T23:30Z is Thursday 00:30 in Berlin (UTC+1 in November).
    const at = new Date("2026-11-04T23:30:00Z");
    expect(nowIn("Europe/Berlin", at)).toEqual({ day: "thursday", minute: 30 });
    expect(nowIn("UTC", at)).toEqual({ day: "wednesday", minute: 23 * 60 + 30 });
  });
});

describe("sleep", () => {
  it("holds while any window is current", () => {
    const nights = [
      { from: "23:00", until: "07:00" },
      { days: ["saturday" as const], from: "13:00", until: "15:00" },
    ];
    expect(isAsleep(nights, "wednesday", 23 * 60 + 30)).toBe(true);
    expect(isAsleep(nights, "thursday", 6 * 60 + 59)).toBe(true);
    expect(isAsleep(nights, "thursday", 7 * 60)).toBe(false);
    expect(isAsleep(nights, "saturday", 14 * 60)).toBe(true);
    expect(isAsleep([], "saturday", 14 * 60)).toBe(false);
  });
});

describe("a schedule", () => {
  const schedule = {
    playlistId: 9,
    slots: [
      { playlistId: 1, days: ["tuesday" as const] },
      { playlistId: 2, from: "07:00", until: "09:00" },
    ],
  };

  it("shows a current slot's playlist in place of the usual one", () => {
    expect(playlistAt(schedule, "wednesday", 8 * 60)).toBe(2);
    expect(playlistAt(schedule, "wednesday", 9 * 60)).toBe(9);
    expect(playlistAt({ slots: [] }, "wednesday", 9 * 60)).toBeUndefined();
  });

  it("finds the first minute of the week two slots share, as the server does", () => {
    expect(overlapOf(schedule.slots)).toEqual({ slots: [0, 1], day: "tuesday", minute: 7 * 60 });
    expect(
      overlapOf([
        { days: ["monday"], from: "01:00", until: "03:00" },
        { days: ["sunday"], from: "22:00", until: "02:00" },
      ]),
      "Sunday night runs into Monday",
    ).toEqual({ slots: [0, 1], day: "monday", minute: 60 });
    expect(
      overlapOf([
        { days: ["monday"], from: "07:00", until: "09:00" },
        { days: ["monday"], from: "09:00", until: "07:00" },
      ]),
      "slots that meet",
    ).toBeUndefined();
  });

  it("lays a day out as what the display shows, sleep over the schedule", () => {
    const plan = { schedule, windows: [{ from: "23:00", until: "07:00" }] };
    expect(dayOf(plan, "wednesday")).toEqual([
      { start: 0, end: 7 * 60, shown: { kind: "asleep" } },
      { start: 7 * 60, end: 9 * 60, shown: { kind: "playlist", playlistId: 2 } },
      { start: 9 * 60, end: 23 * 60, shown: { kind: "playlist", playlistId: 9 } },
      { start: 23 * 60, end: 1440, shown: { kind: "asleep" } },
    ]);
  });

  it("gives a pinned playlist or screen the awake hours, and a held device none", () => {
    const windows = [{ from: "23:00", until: "07:00" }];
    expect(dayOf({ schedule, windows, shows: { playlistId: 5 } }, "wednesday")[1]).toEqual({
      start: 7 * 60,
      end: 23 * 60,
      shown: { kind: "playlist", playlistId: 5 },
    });
    expect(dayOf({ schedule, windows, shows: { screenId: 3 } }, "wednesday")[1]?.shown).toEqual({
      kind: "screen",
      screenId: 3,
    });
    expect(dayOf({ schedule, windows, held: true }, "wednesday")).toEqual([
      { start: 0, end: 1440, shown: { kind: "asleep" } },
    ]);
    expect(dayOf({ schedule: { slots: [] }, windows: [] }, "monday")).toEqual([
      { start: 0, end: 1440, shown: { kind: "nothing" } },
    ]);
  });
});
