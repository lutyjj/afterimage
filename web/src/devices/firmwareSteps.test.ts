import type { Device, FirmwareUpdate } from "@afterimage/management-client";
import { describe, expect, it } from "vitest";

import { nextCheckIn, updateSteps } from "./firmwareSteps";

const SEEN = new Date("2026-11-06T10:00:00Z");

const device: Device = {
  id: 1,
  name: "Office",
  mac: "41:B4:10:39:A1:01",
  firmwareModel: "xiao_epaper_display",
  display: { model: "og_plus", palette: "gray-4" },
  refreshMinutes: 15,
  schedule: { slots: [] },
  shows: { schedule: {} },
  sleep: { windows: [] },
  sleepHeld: false,
  readings: { firmwareVersion: "1.6.10" },
  seenAt: SEEN,
  wakesAt: new Date(SEEN.getTime() + 15 * 60_000),
  firmware: { board: "seeed_OG_DIY_Kit", boardChosen: true, latestVersion: "1.8.18", updateAvailable: true },
};

function update(change: Partial<FirmwareUpdate>): FirmwareUpdate {
  return {
    version: "1.8.18",
    state: "requested",
    requestedAt: new Date("2026-11-06T10:05:00Z"),
    ...change,
  };
}

const states = (steps: ReturnType<typeof updateSteps>) => steps.map((step) => `${step.label}:${step.state}`);

describe("updateSteps", () => {
  it("waits for the next check-in, due a refresh interval after the last", () => {
    const steps = updateSteps(update({}), device);
    expect(states(steps)).toEqual(["Requested:done", "Sent:current", "Installed:waiting"]);
    expect(steps[1]?.at).toEqual(new Date("2026-11-06T10:15:00Z"));
  });

  it("waits for the check-in after the one that carried the request", () => {
    const sentAt = new Date("2026-11-06T10:15:00Z");
    const steps = updateSteps(update({ state: "sent", sentAt }), device);
    expect(states(steps)).toEqual(["Requested:done", "Sent:done", "Installed:current"]);
    expect(steps[1]?.at).toEqual(sentAt);
  });

  it("ends installed or not", () => {
    const sentAt = new Date("2026-11-06T10:15:00Z");
    const finishedAt = new Date("2026-11-06T10:16:00Z");
    expect(states(updateSteps(update({ state: "installed", sentAt, finishedAt }), device))).toEqual([
      "Requested:done",
      "Sent:done",
      "Installed:done",
    ]);
    const failed = updateSteps(update({ state: "not_installed", sentAt, finishedAt }), device);
    expect(states(failed)).toEqual(["Requested:done", "Sent:done", "Not installed:failed"]);
    expect(failed[2]?.at).toEqual(finishedAt);
  });
});

describe("nextCheckIn", () => {
  it("is when the server asked the device to wake, unknown before its first check-in", () => {
    expect(nextCheckIn(device)).toEqual(new Date("2026-11-06T10:15:00Z"));
    expect(nextCheckIn({ ...device, seenAt: undefined, wakesAt: undefined })).toBeUndefined();
  });
});
