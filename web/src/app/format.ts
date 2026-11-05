const relative = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

/** "just now", "4 minutes ago", "yesterday". */
export function ago(date: Date, now: Date = new Date()): string {
  const seconds = Math.round((date.getTime() - now.getTime()) / 1000);
  const abs = Math.abs(seconds);
  if (abs < 45) return "just now";
  if (abs < 3600) return relative.format(Math.round(seconds / 60), "minute");
  if (abs < 86_400) return relative.format(Math.round(seconds / 3600), "hour");
  return relative.format(Math.round(seconds / 86_400), "day");
}

/** A moment in the server's time zone, such as "Thu 14:05" or "3 Nov 14:05". */
export function moment(date: Date, timeZone: string, now: Date = new Date()): string {
  const sameWeek = Math.abs(now.getTime() - date.getTime()) < 6 * 86_400_000;
  return new Intl.DateTimeFormat("en-GB", {
    timeZone,
    ...(sameWeek ? { weekday: "short" } : { day: "numeric", month: "short" }),
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(date);
}

/** A reading of signal strength in words, from RSSI in dBm. */
export function signalQuality(dbm: number): string {
  if (dbm >= -60) return "strong";
  if (dbm >= -70) return "good";
  if (dbm >= -80) return "fair";
  return "weak";
}

export function minutesLabel(minutes: number): string {
  if (minutes % 60 === 0) {
    const hours = minutes / 60;
    return hours === 1 ? "1 hour" : `${hours} hours`;
  }
  return minutes === 1 ? "1 minute" : `${minutes} minutes`;
}
