import type { PlaylistEntry, Weekday } from "@afterimage/management-client";

import { timeOfDay } from "../app/format";

/** The server's rule, in the server's time zone: an entry is current on its days,
 * from `from` until `until`; one whose `until` comes first runs past midnight and
 * belongs to the day it starts; one whose times are equal is current all day. */

export const WEEK: readonly Weekday[] = [
  "monday",
  "tuesday",
  "wednesday",
  "thursday",
  "friday",
  "saturday",
  "sunday",
];

export const DAY_MINUTES = 24 * 60;

export function minuteOf(time: string | undefined): number | undefined {
  if (!time) return undefined;
  const [hours, minutes] = time.split(":").map(Number);
  return (hours ?? 0) * 60 + (minutes ?? 0);
}

function on(entry: PlaylistEntry, day: Weekday) {
  return !entry.days || entry.days.includes(day);
}

function previous(day: Weekday): Weekday {
  return WEEK[(WEEK.indexOf(day) + 6) % 7] ?? day;
}

/** The parts of `day` in which the entry is current, as minute ranges. */
export function spansOn(entry: PlaylistEntry, day: Weekday): Array<[number, number]> {
  const from = minuteOf(entry.from) ?? 0;
  const until = minuteOf(entry.until) ?? DAY_MINUTES;
  if (from === until) return on(entry, day) ? [[0, DAY_MINUTES]] : [];
  if (from < until) return on(entry, day) ? [[from, until]] : [];
  const spans: Array<[number, number]> = [];
  if (on(entry, previous(day))) spans.push([0, until]);
  if (on(entry, day)) spans.push([from, DAY_MINUTES]);
  return spans;
}

export function isCurrent(entry: PlaylistEntry, day: Weekday, minute: number): boolean {
  return spansOn(entry, day).some(([start, end]) => start <= minute && minute < end);
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

/** Whether the entry is current all day on its days. */
export function isAllDay(entry: PlaylistEntry): boolean {
  const from = minuteOf(entry.from) ?? 0;
  const until = minuteOf(entry.until) ?? DAY_MINUTES;
  return from === until || (from === 0 && until === DAY_MINUTES);
}

/** How long the entry runs on one of its days, for a label such as "07:00–09:30". */
export function windowLabel(entry: PlaylistEntry): string {
  if (isAllDay(entry)) return "All day";
  return `${timeOfDay(minuteOf(entry.from) ?? 0)}–${timeOfDay((minuteOf(entry.until) ?? DAY_MINUTES) % DAY_MINUTES)}`;
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
