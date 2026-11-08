import type { Device } from "@afterimage/management-client";

import { hoursLabel } from "../devices/schedule";

/** A display that shows a playlist, and when: pinned, usually, or in the
 * slots that name it. */
export interface Use {
  device: Device;
  when: string[];
}

/** What deleting a playlist does to the displays that show it, as the server's
 * deletion does it; `undefined` until the displays have loaded, when it says
 * what happens without naming anyone. */
export function playlistDeletion(uses: readonly Use[] | undefined): string {
  const effect =
    "Where it was usually shown, nothing is scheduled until another playlist takes its place; a display pinned on it goes back to its schedule.";
  if (uses === undefined) return `It leaves every schedule that shows it. ${effect}`;
  if (uses.length === 0) return "No display shows it.";
  return `It leaves the schedule of ${uses.map(({ device }) => device.name).join(", ")}. ${effect}`;
}

/** The displays that show the playlist, in their order, each with when. */
export function usesOf(playlistId: number, devices: readonly Device[]): Use[] {
  return devices.flatMap((device) => {
    const when = [
      ...(device.shows.playlistId === playlistId ? ["Pinned"] : []),
      ...(device.schedule.playlistId === playlistId ? ["Usually"] : []),
      ...device.schedule.slots.filter((slot) => slot.playlistId === playlistId).map(hoursLabel),
    ];
    return when.length > 0 ? [{ device, when }] : [];
  });
}
