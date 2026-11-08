import type { Schedule, ScheduleSlot, Shows, Weekday } from "@afterimage/management-client";

import { timeOfDay } from "../app/format";

/** The server's rule (crates/server/src/schedule.rs), in the server's time zone,
 * for a schedule slot and a sleep window alike: it is current on its days, from
 * `from` until `until`; one whose `until` comes first runs past midnight and
 * belongs to the day it starts; one whose times are equal is current all day.
 * An awake device shows the playlist of the slot that is current, or else its
 * usual one, and no two slots are current at once. */

/** When something is current: a slot's and a sleep window's hours. */
export type Hours = Pick<ScheduleSlot, "days" | "from" | "until">;

export const WEEK: readonly Weekday[] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

export const DAY_MINUTES = 24 * 60;

export function minuteOf(time: string | undefined): number | undefined {
  if (!time) return undefined;
  const [hours, minutes] = time.split(":").map(Number);
  return (hours ?? 0) * 60 + (minutes ?? 0);
}

function on(hours: Hours, day: Weekday) {
  return !hours.days || hours.days.includes(day);
}

function previous(day: Weekday): Weekday {
  return WEEK[(WEEK.indexOf(day) + 6) % 7] ?? day;
}

/** The parts of `day` in which the hours are current, as minute ranges. */
export function spansOn(hours: Hours, day: Weekday): Array<[number, number]> {
  const from = minuteOf(hours.from) ?? 0;
  const until = minuteOf(hours.until) ?? DAY_MINUTES;
  if (from === until) return on(hours, day) ? [[0, DAY_MINUTES]] : [];
  if (from < until) return on(hours, day) ? [[from, until]] : [];
  const spans: Array<[number, number]> = [];
  if (on(hours, previous(day))) spans.push([0, until]);
  if (on(hours, day)) spans.push([from, DAY_MINUTES]);
  return spans;
}

export function isCurrent(hours: Hours, day: Weekday, minute: number): boolean {
  return spansOn(hours, day).some(([start, end]) => start <= minute && minute < end);
}

/** Whether a device with these sleep windows sleeps at that minute. */
export function isAsleep(windows: readonly Hours[], day: Weekday, minute: number): boolean {
  return windows.some((window) => isCurrent(window, day, minute));
}

/** The playlist the schedule calls for at that minute. */
export function playlistAt(schedule: Schedule, day: Weekday, minute: number): number | undefined {
  return schedule.slots.find((slot) => isCurrent(slot, day, minute))?.playlistId ?? schedule.playlistId;
}

/** Two slots current at one minute of the week, as the server finds them: the
 * first such minute from Monday's midnight. */
export interface Overlap {
  slots: [number, number];
  day: Weekday;
  minute: number;
}

export function overlapOf(slots: readonly Hours[]): Overlap | undefined {
  for (const day of WEEK) {
    for (let minute = 0; minute < DAY_MINUTES; minute++) {
      const current = slots.flatMap((slot, index) => (isCurrent(slot, day, minute) ? [index] : []));
      const [first, second] = current;
      if (first !== undefined && second !== undefined) return { slots: [first, second], day, minute };
    }
  }
  return undefined;
}

/** What a device shows at a moment: a playlist or a pinned screen, its sleep
 * screen, or the server's own saying nothing is scheduled. */
export type Shown =
  | { kind: "playlist"; playlistId: number }
  | { kind: "screen"; screenId: number }
  | { kind: "asleep" }
  | { kind: "nothing" };

/** A run of the day in which a device shows one thing, in minutes. */
export interface Stretch {
  start: number;
  end: number;
  shown: Shown;
}

/** What decides what a device shows: its schedule and sleep windows, and the
 * pin and hold that stand over them. */
export interface Plan {
  schedule: Schedule;
  windows: readonly Hours[];
  shows?: Shows | undefined;
  held?: boolean | undefined;
}

function shownAt(plan: Plan, day: Weekday, minute: number): Shown {
  if (plan.held || isAsleep(plan.windows, day, minute)) return { kind: "asleep" };
  if (plan.shows?.screenId !== undefined) return { kind: "screen", screenId: plan.shows.screenId };
  const playlistId = plan.shows?.playlistId ?? playlistAt(plan.schedule, day, minute);
  return playlistId === undefined ? { kind: "nothing" } : { kind: "playlist", playlistId };
}

function same(one: Shown, other: Shown): boolean {
  switch (one.kind) {
    case "playlist":
      return other.kind === "playlist" && other.playlistId === one.playlistId;
    case "screen":
      return other.kind === "screen" && other.screenId === one.screenId;
    default:
      return other.kind === one.kind;
  }
}

/** What the device shows across `day`, from midnight to midnight, as stretches
 * that each show one thing. */
export function dayOf(plan: Plan, day: Weekday): Stretch[] {
  const stretches: Stretch[] = [];
  for (let minute = 0; minute < DAY_MINUTES; minute++) {
    const shown = shownAt(plan, day, minute);
    const last = stretches.at(-1);
    if (last && same(last.shown, shown)) last.end = minute + 1;
    else stretches.push({ start: minute, end: minute + 1, shown });
  }
  return stretches;
}

/** The weekday and minute of the day now, in `timeZone`. */
export function nowIn(timeZone: string, at: Date = new Date()): { day: Weekday; minute: number } {
  const parts = new Intl.DateTimeFormat("en-GB", {
    timeZone,
    weekday: "long",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).formatToParts(at);
  const part = (type: string) => parts.find((p) => p.type === type)?.value ?? "";
  const day = part("weekday").toLowerCase() as Weekday;
  return { day, minute: Number(part("hour")) * 60 + Number(part("minute")) };
}

/** Whether the hours are current all day on their days. */
export function isAllDay(hours: Hours): boolean {
  const from = minuteOf(hours.from) ?? 0;
  const until = minuteOf(hours.until) ?? DAY_MINUTES;
  return from === until || (from === 0 && until === DAY_MINUTES);
}

/** How long the hours run on one of their days, for a label such as "07:00–09:30". */
export function windowLabel(hours: Hours): string {
  if (isAllDay(hours)) return "All day";
  return `${timeOfDay(minuteOf(hours.from) ?? 0)}–${timeOfDay((minuteOf(hours.until) ?? DAY_MINUTES) % DAY_MINUTES)}`;
}

const SHORT_DAYS: Record<Weekday, string> = {
  monday: "Mon",
  tuesday: "Tue",
  wednesday: "Wed",
  thursday: "Thu",
  friday: "Fri",
  saturday: "Sat",
  sunday: "Sun",
};

export function shortDay(day: Weekday) {
  return SHORT_DAYS[day];
}

/** "Every day", "Weekdays", "Mon, Wed, Fri". */
export function daysLabel(days: Weekday[] | undefined): string {
  if (!days || days.length === 7) return "Every day";
  const sorted = WEEK.filter((day) => days.includes(day));
  if (sorted.length === 5 && !sorted.includes("saturday") && !sorted.includes("sunday")) {
    return "Weekdays";
  }
  if (sorted.length === 2 && sorted.includes("saturday") && sorted.includes("sunday")) {
    return "Weekends";
  }
  return sorted.map(shortDay).join(", ");
}

/** "Weekdays, all day", "Every day, 07:00–09:00": a slot or window in words. */
export function hoursLabel(hours: Hours): string {
  const days = daysLabel(hours.days);
  const window = windowLabel(hours);
  return `${days}, ${window === "All day" ? "all day" : window}`;
}
