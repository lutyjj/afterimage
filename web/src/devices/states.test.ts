import type { Device } from "@afterimage/management-client";
import { describe, expect, it } from "vitest";

import { deviceStates, endedStates, type DeviceState } from "./states";

// Friday 6 November 2026, 12:00 in Berlin.
const NOW = new Date("2026-11-06T11:00:00Z");
const ZONE = "Europe/Berlin";

function device(change: Partial<Device>): Device {
  return {
    id: 1,
    name: "Kitchen",
    mac: "41:B4:10:39:A1:01",
    firmwareModel: "og",
    display: { model: "og_png", palette: "bw" },
    refreshMinutes: 15,
    playlist: [{ screenId: 1 }],
    sleep: { windows: [] },
    readings: {},
    seenAt: new Date(NOW.getTime() - 60_000),
    wakesAt: new Date(NOW.getTime() + 14 * 60_000),
    firmware: { boardChosen: false, updateAvailable: false },
    ...change,
  };
}

const keys = (states: DeviceState[]) => states.map((state) => state.key);

describe("deviceStates", () => {
  it("names nothing for a device that checked in and has a current screen", () => {
    expect(keys(deviceStates(device({}), ZONE, NOW))).toEqual([]);
  });

  it("waits for the first check-in of a device never seen", () => {
    expect(keys(deviceStates(device({ seenAt: undefined }), ZONE, NOW))).toEqual(["never-seen"]);
  });

  it("calls a device quiet past its wake by its interval and ten minutes not seen lately", () => {
    const quiet = (minutes: number) => {
      const seenAt = new Date(NOW.getTime() - minutes * 60_000);
      return device({ seenAt, wakesAt: new Date(seenAt.getTime() + 15 * 60_000) });
    };
    expect(keys(deviceStates(quiet(40), ZONE, NOW))).toEqual([]);
    expect(keys(deviceStates(quiet(41), ZONE, NOW))).toEqual(["not-seen"]);
  });

  it("says nothing is scheduled when no entry is current in the server's time zone", () => {
    expect(keys(deviceStates(device({ playlist: [] }), ZONE, NOW))).toEqual(["nothing-scheduled"]);
    // 08:00 to 11:30 in Berlin has ended by 12:00 there, though not in UTC.
    const morning = device({ playlist: [{ screenId: 1, from: "08:00", until: "11:30" }] });
    expect(keys(deviceStates(morning, ZONE, NOW))).toEqual(["nothing-scheduled"]);
    expect(keys(deviceStates(morning, "UTC", NOW))).toEqual([]);
  });
});

describe("endedStates", () => {
  const state = (key: DeviceState["key"]): DeviceState => ({ key, label: key, title: key });

  it("keeps a state that ended, and drops it when it comes back", () => {
    const ended = endedStates([], [state("never-seen"), state("nothing-scheduled")], [state("nothing-scheduled")]);
    expect(keys(ended)).toEqual(["never-seen"]);
    expect(keys(endedStates(ended, [state("nothing-scheduled")], [state("nothing-scheduled")]))).toEqual([
      "never-seen",
    ]);
    expect(keys(endedStates(ended, [], [state("never-seen")]))).toEqual([]);
  });

  it("lets a device sleep through the night unmissed, with nothing missing from its playlist", () => {
    // 06:30 Friday in Berlin: asleep since its check-in at 23:00 Thursday, due at
    // 07:00. Counted from the check-in alone it would be hours overdue.
    const dawn = new Date("2026-11-06T05:30:00Z");
    const sleeping = device({
      seenAt: new Date("2026-11-05T22:00:00Z"),
      wakesAt: new Date("2026-11-06T06:00:00Z"),
      playlist: [{ screenId: 1, from: "08:00", until: "20:00" }],
      sleep: { windows: [{ from: "23:00", until: "07:00" }] },
    });
    expect(keys(deviceStates(sleeping, ZONE, dawn))).toEqual([]);
    expect(
      keys(deviceStates({ ...sleeping, sleep: { windows: [] } }, ZONE, dawn)),
      "awake at that hour, its playlist has nothing current",
    ).toEqual(["nothing-scheduled"]);
  });
});
