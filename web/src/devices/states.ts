import type { Device } from "@afterimage/management-client";
import { useRef } from "react";

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

export interface StampedState extends DeviceState {
  cancelled: boolean;
}

/** The device's states as stamps that cancel rather than vanish: a state that ends
 * while the page is open stays, struck through. */
export function useStamps(states: DeviceState[]): StampedState[] {
  const seen = useRef(new Map<string, DeviceState>());
  for (const state of states) seen.current.set(state.key, state);
  const live = new Set(states.map((state) => state.key));
  return [...seen.current.values()].map((state) => ({ ...state, cancelled: !live.has(state.key) }));
}
