import type { Playlist, ScreenSummary } from "@afterimage/management-client";
import { classes } from "linen";

import { timeOfDay } from "../app/format";
import { DAY_MINUTES, type Stretch } from "./schedule";

/** One stretch of a day on a track, in minutes: what the display shows then,
 * its sleep, or the server's own screen saying nothing is scheduled. */
export interface Mark {
  start: number;
  end: number;
  label: string;
  tone: "shown" | "asleep" | "nothing";
}

/** Names for what a stretch shows. */
export interface Names {
  playlist: (id: number) => string;
  screen: (id: number) => string;
}

/** Names for what a display shows, from the account's playlists and screens. */
export function namesOf(
  playlists: readonly Playlist[] | undefined,
  screens: readonly ScreenSummary[] | undefined,
): Names {
  return {
    playlist: (id) => playlists?.find((playlist) => playlist.id === id)?.name ?? "A playlist",
    screen: (id) => screens?.find((screen) => screen.id === id)?.name ?? "A screen",
  };
}

/** The marks of a day of stretches, each named. */
export function marksOf(stretches: readonly Stretch[], names: Names): Mark[] {
  return stretches.map(({ start, end, shown }) => {
    switch (shown.kind) {
      case "playlist":
        return { start, end, label: names.playlist(shown.playlistId), tone: "shown" };
      case "screen":
        return { start, end, label: names.screen(shown.screenId), tone: "shown" };
      case "asleep":
        return { start, end, label: "Asleep", tone: "asleep" };
      case "nothing":
        return { start, end, label: "Nothing", tone: "nothing" };
    }
  });
}

const percent = (minute: number) => `${(minute / DAY_MINUTES) * 100}%`;

/** What the display shows is lit, as paper; asleep it is dark, the bare track;
 * nothing scheduled is a state that wants the person, in warn. */
const TONES: Record<Mark["tone"], string> = {
  shown: "border border-border-strong bg-surface text-text-secondary",
  asleep: "text-text-muted",
  nothing: "border border-dashed border-warn/60 text-warn",
};

/** "Asleep until 07:00, Weekdays 07:00–23:00, Asleep from 23:00", for a reader. */
function spoken(marks: readonly Mark[]): string {
  return marks
    .map(({ start, end, label }) => {
      if (start === 0 && end === DAY_MINUTES) return `${label} all day`;
      if (start === 0) return `${label} until ${timeOfDay(end)}`;
      if (end === DAY_MINUTES) return `${label} from ${timeOfDay(start)}`;
      return `${label} ${timeOfDay(start)}–${timeOfDay(end)}`;
    })
    .join(", ");
}

/** One day from midnight to midnight, each stretch as long as it lasts and
 * named where it has the room, a line at now when the track is today's. The
 * track reports; it never takes the ink. */
export function DayTrack({
  marks,
  now,
  label,
}: {
  marks: readonly Mark[];
  /** The minute of the day to mark as now. */
  now?: number | undefined;
  /** What the track is of, such as its day; the stretches follow it. */
  label: string;
}) {
  return (
    <div
      role="img"
      aria-label={marks.length > 0 ? `${label}: ${spoken(marks)}` : label}
      className="relative h-6 rounded-input bg-sunken"
    >
      {[3, 6, 9, 12, 15, 18, 21].map((hour) => (
        <span
          key={hour}
          aria-hidden="true"
          className={classes("absolute inset-y-0 w-px", hour % 6 === 0 ? "bg-border-strong" : "bg-border/60")}
          style={{ left: percent(hour * 60) }}
        />
      ))}
      {marks.map((mark) => (
        <span
          key={mark.start}
          title={`${mark.label}, ${timeOfDay(mark.start)}–${timeOfDay(mark.end % DAY_MINUTES)}`}
          // Each stretch is a container, so its name shows only where it fits.
          className={classes(
            "@container absolute inset-y-[3px] flex min-w-0.5 items-center overflow-hidden rounded-[4px] text-xs leading-none",
            TONES[mark.tone],
          )}
          // A pixel apart from its neighbours, so two stretches read as two.
          style={{
            left: `calc(${percent(mark.start)} + 1px)`,
            width: `calc(${percent(mark.end - mark.start)} - 2px)`,
          }}
        >
          <span className="hidden truncate px-1.5 @min-[2.75rem]:block">{mark.label}</span>
        </span>
      ))}
      {now !== undefined && (
        <span
          aria-hidden="true"
          className="absolute -inset-y-1 -ml-px w-0.5 rounded-[1px] bg-text"
          style={{ left: percent(now) }}
        />
      )}
    </div>
  );
}

/** The hours under a track, at its edges and every six hours between. */
export function HourAxis() {
  return (
    <div aria-hidden="true" className="relative h-4 text-xs text-text-muted tabular-nums">
      {[0, 6, 12, 18, 24].map((hour, index, all) => (
        <span
          key={hour}
          className={classes(
            "absolute",
            index === 0 ? "" : index === all.length - 1 ? "-translate-x-full" : "-translate-x-1/2",
          )}
          style={{ left: percent(hour * 60) }}
        >
          {String(hour).padStart(2, "0")}
        </span>
      ))}
    </div>
  );
}
