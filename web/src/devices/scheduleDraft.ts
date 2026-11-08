import { type Hours, isAllDay } from "./schedule";

/** A schedule slot or sleep window while it is edited. */
export type Draft<T extends Hours> = T & {
  key: number;
  /** Chosen with the row's checkbox, so times typed equal on the way to another
   * value do not make it all day. */
  allDay: boolean;
};

let nextKey = 0;

export function newKey(): number {
  return nextKey++;
}

/** Drafts of saved items. Made again from what the drafts became, as a save
 * returns them in the drafts' order, each row keeps its key, so the rows on the
 * page stay the same rows rather than all arriving anew. */
export function draftOf<T extends Hours>(items: readonly T[], before: readonly Draft<T>[] = []): Draft<T>[] {
  return items.map((item, index) => ({ ...item, key: before[index]?.key ?? newKey(), allDay: isAllDay(item) }));
}

/** What a draft saves as: an all-day one without its times. */
export function entryOf<T extends Hours>({ key: _key, allDay, ...item }: Draft<T>): T {
  const saved = item as unknown as T;
  return allDay ? { ...saved, from: undefined, until: undefined } : saved;
}

/** Whether drafts say what the saved items say, each read the way the editor
 * saves it, so a saved item whose equal times make it all day matches its draft. */
export function sameAs<T extends Hours>(drafts: readonly Draft<T>[], saved: readonly T[]): boolean {
  return JSON.stringify(drafts.map(entryOf)) === JSON.stringify(draftOf(saved).map(entryOf));
}
