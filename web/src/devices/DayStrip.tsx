import type { PlaylistEntry, Weekday } from "@afterimage/management-client";

import { DAY_MINUTES, spansOn } from "./schedule";
import styles from "./DayStrip.module.css";

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
 * marks are as long as the time it is current, its name at its start. */
export function DayStrip({ entries, day, now, hours = true, names = true, label }: DayStripProps) {
  return (
    <div className={styles.strip} role="img" aria-label={label} data-named={names || undefined}>
      {names && (
        <ol className={styles.names} aria-hidden="true">
          {entries.map(({ name }, lane) => (
            <li key={lane} title={name}>
              {name}
            </li>
          ))}
        </ol>
      )}
      <div className={styles.track}>
        {[3, 6, 9, 12, 15, 18, 21].map((hour) => (
          <span
            key={hour}
            className={styles.hour}
            data-major={hour % 6 === 0 || undefined}
            style={{ left: percent(hour * 60) }}
          />
        ))}
        {entries.map(({ entry, name }, lane) => (
          <div key={lane} className={styles.lane}>
            {spansOn(entry, day).map(([start, end]) => (
              <span
                key={start}
                className={styles.span}
                style={{ left: percent(start), width: percent(end - start) }}
                title={name || undefined}
              />
            ))}
          </div>
        ))}
        {entries.length === 0 && <div className={styles.lane} />}
        {now !== undefined && <span className={styles.now} style={{ left: percent(now) }} />}
      </div>
      {hours && (
        <div className={styles.scale} aria-hidden="true">
          {[0, 6, 12, 18, 24].map((hour) => (
            <span key={hour} style={{ left: percent(hour * 60) }}>
              {String(hour).padStart(2, "0")}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
