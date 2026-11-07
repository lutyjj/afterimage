import type { Device } from "@afterimage/management-client";
import { useEffect, useRef, useState } from "react";

import { ago } from "../app/format";
import { isCurrent, nowIn } from "./schedule";

export interface DeviceState {
  key: "never-seen" | "not-seen" | "nothing-scheduled";
  label: string;
  title: string;
}

/** The states that need the person's attention. A device misses a check-in when it
 * stays quiet past twice its refresh interval and a few minutes more. */
export function deviceStates(device: Device, timeZone: string, now: Date = new Date()): DeviceState[] {
  const states: DeviceState[] = [];
  if (!device.seenAt) {
    states.push({
      key: "never-seen",
      label: "Not checked in",
      title: "Claimed, and waiting for the display's first check-in.",
    });
  } else {
    const quietFor = now.getTime() - device.seenAt.getTime();
    if (quietFor > (device.refreshMinutes * 2 + 10) * 60_000) {
      states.push({
        key: "not-seen",
        label: "Not seen lately",
        title: `Last check-in ${ago(device.seenAt, now)}. The display may be off, out of range or out of battery.`,
      });
    }
  }
  const { day, minute } = nowIn(timeZone, now);
  if (!device.playlist.some((entry) => isCurrent(entry, day, minute))) {
    states.push({
      key: "nothing-scheduled",
      label: "Nothing scheduled",
      title:
        device.playlist.length === 0
          ? "The playlist is empty, so the display shows “Nothing to show”."
          : "No playlist entry is current now, so the display shows “Nothing to show”.",
    });
  }
  return states;
}

export interface ShownState extends DeviceState {
  ended: boolean;
}

const ORDER: Array<DeviceState["key"]> = ["never-seen", "not-seen", "nothing-scheduled"];

/** The states that have ended while the page was open, after the device moved
 * from `before` to `now`: those that ended earlier and are still over, and those
 * that just ended. */
export function endedStates(ended: DeviceState[], before: DeviceState[], now: DeviceState[]): DeviceState[] {
  const live = new Set(now.map((state) => state.key));
  const over = [...ended, ...before].filter((state) => !live.has(state.key));
  return [...new Map(over.map((state) => [state.key, state])).values()];
}

/** The device's states as the page shows them: a state that ends while the page
 * is open stays, struck through, rather than vanishing. */
export function useShownStates(states: DeviceState[]): ShownState[] {
  const [ended, setEnded] = useState<DeviceState[]>([]);
  const before = useRef(states);
  const key = states.map((state) => state.key).join();
  useEffect(() => {
    setEnded((current) => endedStates(current, before.current, states));
    before.current = states;
    // `key` names the states; the array is new on every render.
  }, [key]);
  const live = new Set(states.map((state) => state.key));
  return [
    ...states.map((state) => ({ ...state, ended: false })),
    ...ended.filter((state) => !live.has(state.key)).map((state) => ({ ...state, ended: true })),
  ].sort((a, b) => ORDER.indexOf(a.key) - ORDER.indexOf(b.key));
}
