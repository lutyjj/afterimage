import type { Weekday } from "@afterimage/management-client";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { CellInput, Checkbox, classes } from "linen";

import { DAY_NAMES, daysLabel, type Hours, isAllDay, shortDay, WEEK } from "./schedule";

type Change = Partial<Hours & { allDay: boolean }>;

/** When a schedule slot or a sleep window is current: its days, then all day or
 * the times between. A fragment, so the row that holds it lays the two out;
 * `timed` is the window an item takes when it stops being all day. */
export function HoursFields({
  hours,
  timed,
  daysClassName,
  timesClassName,
  onChange,
}: {
  hours: Hours & { allDay: boolean };
  timed: { from: string; until: string };
  /** For the days and for the times, which the row places. */
  daysClassName?: string;
  timesClassName?: string;
  onChange: (change: Change) => void;
}) {
  const days = hours.days ?? [...WEEK];
  const { allDay } = hours;

  return (
    <>
      {/* Any number of days at once, which linen's segmented control, one
          answer of a few, does not take: Base UI's toggle group, one stop on
          the Tab key with the arrows between days, in that control's frame,
          type and chosen tint. Its days share the line it is given, down to a
          phone's. */}
      <ToggleGroup
        multiple
        aria-label={`Days: ${daysLabel(hours.days)}`}
        value={days}
        onValueChange={(next) => {
          const chosen = WEEK.filter((day) => next.includes(day));
          onChange({ days: chosen.length === 7 ? undefined : chosen });
        }}
        className={classes(
          "flex h-(--height-control) min-w-0 items-stretch rounded-button border border-border-strong bg-surface p-0.5",
          daysClassName,
        )}
      >
        {WEEK.map((day) => (
          <Toggle
            key={day}
            value={day}
            aria-label={DAY_NAMES[day]}
            className="min-w-0 flex-1 cursor-pointer rounded-input px-2 text-sm text-text-secondary hover:text-text data-pressed:bg-accent-weak data-pressed:font-medium data-pressed:text-accent"
          >
            {shortDay(day).slice(0, 2)}
          </Toggle>
        ))}
      </ToggleGroup>
      <div className={classes("flex min-w-0 flex-wrap items-center gap-2", timesClassName)}>
        <Checkbox
          checked={allDay}
          onCheckedChange={(checked) =>
            onChange(checked ? { allDay: true } : { allDay: false, ...(isAllDay(hours) ? timed : {}) })
          }
          className="w-auto"
        >
          All day
        </Checkbox>
        {!allDay && (
          // The two times keep one line, giving way together on a narrow one,
          // so "to" never stands alone.
          <span className="flex min-w-0 items-center gap-2">
            <CellInput
              aria-label="From"
              className="w-32 min-w-0 shrink"
              type="time"
              step={60}
              value={hours.from ?? "00:00"}
              onChange={(event) => onChange({ from: event.target.value || undefined })}
            />
            <span aria-hidden="true" className="text-sm text-text-muted">
              to
            </span>
            <CellInput
              aria-label="Until"
              className="w-32 min-w-0 shrink"
              type="time"
              step={60}
              value={hours.until ?? "00:00"}
              onChange={(event) => onChange({ until: event.target.value || undefined })}
            />
          </span>
        )}
      </div>
    </>
  );
}
