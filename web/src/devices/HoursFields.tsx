import type { Weekday } from "@afterimage/management-client";
import { CellInput, Checkbox, classes } from "linen";

import { daysLabel, type Hours, isAllDay, shortDay, WEEK } from "./schedule";

type Change = Partial<Hours & { allDay: boolean }>;

/** When a schedule slot or a sleep window is current: its days, then all day or
 * the times between. A fragment, so the row that holds it lays the two out;
 * `timed` is the window an item takes when it stops being all day. */
export function HoursFields({
  hours,
  timed,
  className,
  onChange,
}: {
  hours: Hours & { allDay: boolean };
  timed: { from: string; until: string };
  /** For the times' line, which the row places. */
  className?: string;
  onChange: (change: Change) => void;
}) {
  const days = hours.days ?? [...WEEK];
  const toggle = (day: Weekday) => {
    const next = days.includes(day) ? days.filter((d) => d !== day) : WEEK.filter((d) => d === day || days.includes(d));
    onChange({ days: next.length === 7 ? undefined : next });
  };
  const { allDay } = hours;

  return (
    <>
      <fieldset className="m-0 inline-flex h-(--height-control) w-fit items-stretch gap-0.5 rounded-button border border-border-strong bg-surface p-0.5">
        <legend className="sr-only">Days: {daysLabel(hours.days)}</legend>
        {WEEK.map((day) => {
          const on = days.includes(day);
          return (
            <button
              key={day}
              type="button"
              aria-pressed={on}
              onClick={() => toggle(day)}
              title={day}
              className={classes(
                "w-8 cursor-pointer rounded-input text-xs",
                on ? "bg-accent-weak font-semibold text-accent" : "text-text-secondary hover:text-text",
              )}
            >
              {shortDay(day).slice(0, 2)}
            </button>
          );
        })}
      </fieldset>
      <div className={classes("flex flex-wrap items-center gap-2", className)}>
        <Checkbox
          checked={allDay}
          onCheckedChange={(checked) =>
            onChange(checked ? { allDay: true } : { allDay: false, ...(isAllDay(hours) ? timed : {}) })
          }
          className="w-auto pr-2"
        >
          All day
        </Checkbox>
        {!allDay && (
          <span className="inline-flex flex-wrap items-center gap-2">
            <CellInput
              aria-label="From"
              className="w-32"
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
              className="w-32"
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
