import { describe, expect, it } from "vitest";

import { pinOf, pinValue } from "./pin";

describe("the Showing picker's value", () => {
  it("names each kind of pin apart and reads back to it", () => {
    for (const [shows, value] of [
      [{ schedule: {} }, ""],
      [{ playlistId: 7 }, "playlist:7"],
      [{ screenId: 7 }, "screen:7"],
    ] as const) {
      expect(pinValue(shows)).toBe(value);
      expect(pinOf(value)).toEqual(shows);
    }
  });

  it("reads anything else as the schedule", () => {
    expect(pinOf("playlist:")).toEqual({ schedule: {} });
    expect(pinOf("other:3")).toEqual({ schedule: {} });
  });
});
