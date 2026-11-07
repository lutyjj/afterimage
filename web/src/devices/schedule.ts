import type { PlaylistEntry, Weekday } from "@afterimage/management-client";

import { timeOfDay } from "../app/format";

/** The server's rule, in the server's time zone, for a playlist entry and a sleep
 * window alike: it is current on its days, from `from` until `until`; one whose
 * `until` comes first runs past midnight and belongs to the day it starts; one
 * whose times are equal is current all day. */

/** When something is current: a playlist entry's and a sleep window's hours. */
export type Hours = Pick<PlaylistEntry, "days" | "from" | "until">;

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
