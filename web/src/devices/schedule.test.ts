import { describe, expect, it } from "vitest";

import { isAsleep, isCurrent, nowIn, spansOn } from "./schedule";

// Cases from the server's own rule (crates/server/src/schedule.rs).
describe("the rule for playlist entries and sleep windows", () => {
  it("makes an entry without days or times current all day, every day", () => {
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

  it("runs an entry past midnight on the day it starts", () => {
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
