import type { PlaylistEntry } from "@afterimage/management-client";

import { isAllDay } from "./schedule";

/** A playlist entry while it is edited. */
export interface Draft extends PlaylistEntry {
  key: number;
  /** Chosen with the row's checkbox, so times typed equal on the way to another
   * value do not make the entry all day. */
  allDay: boolean;
}

let nextKey = 0;

export function newKey(): number {
  return nextKey++;
}

/** The draft of a playlist. Made again from a playlist the draft became, as a
 * save returns it in the draft's order, each row keeps its key, so the rows on
 * the page stay the same rows rather than all arriving anew. */
export function draftOf(entries: PlaylistEntry[], before: readonly Draft[] = []): Draft[] {
  return entries.map((entry, index) => ({ ...entry, key: before[index]?.key ?? newKey(), allDay: isAllDay(entry) }));
}

/** The entry a draft saves as: an all-day one without its times. */
export function entryOf({ key: _key, allDay, ...entry }: Draft): PlaylistEntry {
  return allDay ? { ...entry, from: undefined, until: undefined } : entry;
}

/** Whether a draft says what a saved playlist says, each read the way the editor
 * saves it, so a saved entry whose equal times make it all day matches its draft. */
export function sameAs(draft: Draft[], saved: PlaylistEntry[]): boolean {
  return JSON.stringify(draft.map(entryOf)) === JSON.stringify(draftOf(saved).map(entryOf));
}
