import type { Device, Playlist } from "@afterimage/management-client";
import { useEffect, useRef, useState } from "react";

import { ago } from "../app/format";
import { isAsleep, nowIn, playlistAt } from "./schedule";

export interface DeviceState {
  key: "never-seen" | "not-seen" | "nothing-scheduled" | "empty-playlist";
  label: string;
  title: string;
}

/** The states that need the person's attention. A device is not seen lately once it
 * stays quiet past the moment the server names (`overdueAt`, its wake and a little
 * more), so a sleeping device is not missed; Home Assistant reads the same moment.
 * A sleeping device, or one pinned to a screen, misses nothing in its schedule;
 * a playlist not yet read is not taken for empty. */
export function deviceStates(
  device: Device,
  timeZone: string,
  playlists: readonly Playlist[] | undefined,
  now: Date = new Date(),
): DeviceState[] {
  const states: DeviceState[] = [];
  if (!device.seenAt) {
    states.push({
      key: "never-seen",
      label: "Not checked in",
      title: "Claimed, and waiting for the display's first check-in.",
    });
  } else {
    if (device.overdueAt && now > device.overdueAt) {
      states.push({
        key: "not-seen",
        label: "Not seen lately",
        title: `Last check-in ${ago(device.seenAt, now)}. The display may be off, out of range or out of battery.`,
      });
    }
  }
  const { day, minute } = nowIn(timeZone, now);
  const asleep = device.sleepHeld || isAsleep(device.sleep.windows, day, minute);
  if (asleep || device.shows.screenId !== undefined) return states;
  const playlistId = device.shows.playlistId ?? playlistAt(device.schedule, day, minute);
  const playlist = playlists?.find((candidate) => candidate.id === playlistId);
  if (playlistId === undefined) {
    states.push({
      key: "nothing-scheduled",
      label: "Nothing scheduled",
      title: "No playlist is scheduled now, so the display shows “Nothing to show”.",
    });
  } else if (playlist?.screens.length === 0) {
    states.push({
      key: "empty-playlist",
      label: "Empty playlist",
      title: `${playlist.name} has no screens, so the display shows “Nothing to show”.`,
    });
  }
  return states;
}

export interface ShownState extends DeviceState {
  ended: boolean;
}

const ORDER: Array<DeviceState["key"]> = ["never-seen", "not-seen", "nothing-scheduled", "empty-playlist"];

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
