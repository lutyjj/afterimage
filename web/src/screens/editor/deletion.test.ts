import type { Device, Playlist } from "@afterimage/management-client";
import { describe, expect, it } from "vitest";

import { screenDeletion } from "./deletion";

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

describe("deleting a screen", () => {
  const playlists: Playlist[] = [
    { id: 1, name: "Mornings", screens: [3, 4] },
    { id: 2, name: "Clock", screens: [3, 3] },
    { id: 3, name: "Other", screens: [4] },
  ];

  it("says which playlists it leaves, which it empties, and which displays it moves", () => {
    const devices = [
      device(1, { name: "Kitchen", shows: { screenId: 3 } }),
      device(2, { name: "Hallway", sleep: { windows: [], screenId: 3 } }),
      device(3, { name: "Office" }),
    ];
    expect(screenDeletion(3, playlists, devices)).toBe(
      "It leaves the playlist Mornings, whose displays move on to their other screens. " +
        "Clock is left with no screens, so its displays show “Nothing to show”. " +
        "Kitchen goes back to its schedule. " +
        "Hallway sleeps on Afterimage's own sleep screen instead.",
    );
    expect(screenDeletion(9, playlists, devices)).toBe("No playlist or display uses it.");
  });

  it("names no one before the playlists and displays have loaded", () => {
    expect(screenDeletion(3, undefined, [])).toMatch(/^It leaves every playlist that holds it/);
  });
});
