import type { PlaylistEntry, Weekday } from "@afterimage/management-client";
import { classes } from "linen";

import { DAY_MINUTES, spansOn } from "./schedule";

export interface DayStripProps {
  entries: Array<{ entry: PlaylistEntry; name: string }>;
  day: Weekday;
  /** The minute of the day to mark as now, when the strip shows today. */
  now?: number | undefined;
  /** Hour labels under the track. */
  hours?: boolean;
  /** A column of the lanes' names before the track, kept at one width even with
   * no lanes, so strips stacked down a page start their tracks at one edge. */
  names?: boolean;
  label: string;
}

const percent = (minute: number) => `${(minute / DAY_MINUTES) * 100}%`;

/** One day as a track from midnight to midnight, each playlist entry a lane whose
 * marks are as long as the time it is current, its name at its start. The strip
 * reports; it never takes the ink. */
export function DayStrip({ entries, day, now, hours = true, names = true, label }: DayStripProps) {
  return (
    <div
      role="img"
      aria-label={label}
      className={classes(
        "grid gap-x-3 gap-y-1 text-xs",
        names ? "grid-cols-[6.5rem_minmax(0,1fr)]" : "grid-cols-[minmax(0,1fr)]",
      )}
    >
      {names && (
        <ol aria-hidden="true" className="grid auto-rows-[1.25rem] text-text-secondary">
          {entries.map(({ name }, lane) => (
            <li key={lane} title={name} className="truncate leading-5">
              {name}
            </li>
          ))}
        </ol>
      )}
      <div className="relative grid auto-rows-[1.25rem] rounded-input bg-sunken">
        {[3, 6, 9, 12, 15, 18, 21].map((hour) => (
          <span
            key={hour}
            className={classes("absolute inset-y-0 w-px", hour % 6 === 0 ? "bg-border-strong" : "bg-border/60")}
            style={{ left: percent(hour * 60) }}
          />
        ))}
        {entries.map(({ entry, name }, lane) => (
          <div key={lane} className="relative">
            {spansOn(entry, day).map(([start, end]) => (
              <span
                key={start}
                className="absolute inset-y-[5px] min-w-0.5 rounded-full bg-[color-mix(in_oklch,var(--color-text-muted)_70%,var(--color-surface))]"
                style={{ left: percent(start), width: percent(end - start) }}
                title={name || undefined}
              />
            ))}
          </div>
        ))}
        {entries.length === 0 && <div />}
        {now !== undefined && (
          <span className="absolute -inset-y-1 -ml-px w-0.5 rounded-[1px] bg-text" style={{ left: percent(now) }} />
        )}
      </div>
      {hours && (
        <div aria-hidden="true" className="relative col-[-2/-1] h-4 text-text-muted tabular-nums">
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
      )}
    </div>
  );
}
