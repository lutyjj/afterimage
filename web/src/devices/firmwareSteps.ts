import type { Device, FirmwareUpdate } from "@afterimage/management-client";

/** Where a step of an install request stands. */
export type StepState = "done" | "current" | "waiting" | "failed";

export interface Step {
  label: string;
  state: StepState;
  /** When the step happened, or for one still to come, when it is due. */
  at?: Date | undefined;
  /** Words for a step with no time. */
  note?: string | undefined;
}

/** When the device checks in next at the latest: its last check-in and its
 * refresh interval. A playlist change can bring it sooner. */
export function nextCheckIn(device: Device): Date | undefined {
  if (!device.seenAt) return undefined;
  return new Date(device.seenAt.getTime() + device.refreshMinutes * 60_000);
}

/** The three steps an install request goes through: the request, the check-in
 * whose answer carries it, and the check-in after it, which tells whether the
 * device installed the release. */
export function updateSteps(update: FirmwareUpdate, device: Device): Step[] {
  const requested: Step = { label: "Requested", state: "done", at: update.requestedAt };
  switch (update.state) {
    case "requested":
      return [
        requested,
        { label: "Sent", state: "current", at: nextCheckIn(device), note: "At its next check-in" },
        { label: "Installed", state: "waiting", note: "When it checks in again" },
      ];
    case "sent":
      return [
        requested,
        { label: "Sent", state: "done", at: update.sentAt },
        { label: "Installed", state: "current", note: "When it checks in again" },
      ];
    case "installed":
      return [
        requested,
        { label: "Sent", state: "done", at: update.sentAt },
        { label: "Installed", state: "done", at: update.finishedAt },
      ];
    default:
      return [
        requested,
        { label: "Sent", state: "done", at: update.sentAt },
        { label: "Not installed", state: "failed", at: update.finishedAt },
      ];
  }
}
