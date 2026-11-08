import type { Device } from "@afterimage/management-client";
import { describe, expect, it } from "vitest";

import { playlistDeletion, usesOf } from "./usedBy";

function device(id: number, change: Partial<Device>): Device {
  return {
    id,
    name: `Display ${id}`,
    mac: "41:B4:10:39:A1:01",
    firmwareModel: "og",
    display: { model: "og_png", palette: "bw" },
    refreshMinutes: 15,
    schedule: { slots: [] },
    shows: { schedule: {} },
    sleep: { windows: [] },
    sleepHeld: false,
    readings: {},
    firmware: { boardChosen: false, updateAvailable: false },
    ...change,
  };
}

describe("the displays a playlist serves", () => {
  it("names each display once, with every way it shows the playlist", () => {
    const devices = [
      device(1, { schedule: { playlistId: 7, slots: [] } }),
      device(2, {
        shows: { playlistId: 7 },
        schedule: {
          playlistId: 3,
          slots: [
            { playlistId: 7, days: ["monday", "tuesday", "wednesday", "thursday", "friday"] },
            { playlistId: 3, days: ["saturday"] },
            { playlistId: 7, days: ["sunday"] },
          ],
        },
      }),
      device(3, { schedule: { playlistId: 3, slots: [] } }),
    ];
    expect(usesOf(7, devices).map(({ device, when }) => [device.id, when])).toEqual([
      [1, ["Usually"]],
      [2, ["Pinned", "Weekdays, all day", "Sun, all day"]],
    ]);
  });
});

describe("deleting a playlist", () => {
  it("names the displays it leaves, and no one before they have loaded", () => {
    const kitchen = device(1, { name: "Kitchen", schedule: { playlistId: 7, slots: [] } });
    expect(playlistDeletion(usesOf(7, [kitchen]))).toMatch(/^It leaves the schedule of Kitchen\./);
    expect(playlistDeletion([])).toBe("No display shows it.");
    expect(playlistDeletion(undefined)).toMatch(/^It leaves every schedule that shows it\./);
  });
});
