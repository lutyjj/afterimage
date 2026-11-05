import type { Device, PlaylistEntry, ScreenSummary, Weekday } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, Plus, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { Link } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import { DayStrip } from "./DayStrip";
import styles from "./PlaylistEditor.module.css";
import { daysLabel, nowIn, shortDay, WEEK, windowLabel } from "./schedule";

interface Draft extends PlaylistEntry {
  key: number;
}

let nextKey = 0;
const draftOf = (entries: PlaylistEntry[]): Draft[] =>
  entries.map((entry) => ({ ...entry, key: nextKey++ }));
const entryOf = ({ key: _key, ...entry }: Draft): PlaylistEntry => entry;
const same = (a: PlaylistEntry[], b: PlaylistEntry[]) => JSON.stringify(a) === JSON.stringify(b);

/** The screens a device shows, in order, each on its days and hours. Every entry
 * draws as a strip of the day, as long as the time it is current. */
export function PlaylistEditor({
  device,
  screens,
  timeZone,
}: {
  device: Device;
  screens: ScreenSummary[] | undefined;
  timeZone: string | undefined;
}) {
  const [draft, setDraft] = useState(() => draftOf(device.playlist));
  const saved = device.playlist;
  const dirty = !same(draft.map(entryOf), saved);

  // A playlist saved elsewhere replaces an untouched draft.
  const savedKey = JSON.stringify(saved);
  const previous = useRef(saved);
  useEffect(() => {
    const before = previous.current;
    previous.current = saved;
    setDraft((current) => (same(current.map(entryOf), before) ? draftOf(saved) : current));
    // `saved` is a new array on every poll; its content is what changes.
  }, [savedKey]);

  const save = useMutation({
    mutationFn: () => client.setPlaylist({ deviceId: device.id, playlist: draft.map(entryOf) }),
    onSuccess: ({ device: updated }) => {
      queryClient.setQueryData(["devices", updated.id], updated);
      void queryClient.invalidateQueries({ queryKey: ["devices"] });
      setDraft(draftOf(updated.playlist));
    },
  });

  const update = (key: number, change: Partial<PlaylistEntry>) =>
    setDraft((entries) => entries.map((entry) => (entry.key === key ? { ...entry, ...change } : entry)));
  const move = (index: number, by: number) =>
    setDraft((entries) => {
      const next = [...entries];
      const [entry] = next.splice(index, 1);
      if (entry) next.splice(index + by, 0, entry);
      return next;
    });
  const remove = (key: number) => setDraft((entries) => entries.filter((entry) => entry.key !== key));
  const add = () => {
    const unused = screens?.find((screen) => !draft.some((entry) => entry.screenId === screen.id));
    const screen = unused ?? screens?.[0];
    if (screen) setDraft((entries) => [...entries, { key: nextKey++, screenId: screen.id }]);
  };

  const name = (id: number) => screens?.find((screen) => screen.id === id)?.name ?? "A deleted screen";
  const now = timeZone ? nowIn(timeZone) : undefined;
  const noDays = draft.some((entry) => entry.days?.length === 0);

  return (
    <section className={styles.section} aria-labelledby="playlist-title">
      <div className={styles.head}>
        <h2 id="playlist-title" className={styles.title}>
          Playlist
        </h2>
        <p className="hint">
          At each check-in the display shows the next screen that is current, in this order.
          {timeZone && <> Times are in {timeZone}.</>}
        </p>
      </div>

      {now && (
        <div className={styles.today}>
          <span className="annotation">Today · {now.day}</span>
          <DayStrip
            day={now.day}
            now={now.minute}
            entries={draft.map((entry) => ({ entry, name: name(entry.screenId) }))}
            label="Today's playlist"
          />
        </div>
      )}

      {screens?.length === 0 ? (
        <p className={styles.empty}>
          No screens yet. <Link to="/screens/new">Write the first screen</Link>, then come back to
          give it a place here.
        </p>
      ) : (
        <>
          {draft.length === 0 && (
            <p className={styles.empty}>
              Nothing scheduled: the display shows “Nothing to show”. Add a screen to give it
              something.
            </p>
          )}
          <ol className={styles.entries}>
            {draft.map((entry, index) => (
              <EntryRow
                key={entry.key}
                entry={entry}
                index={index}
                count={draft.length}
                screens={screens ?? []}
                onChange={(change) => update(entry.key, change)}
                onMove={(by) => move(index, by)}
                onRemove={() => remove(entry.key)}
              />
            ))}
          </ol>
          <div className={styles.actions}>
            <button className="button" type="button" onClick={add} disabled={!screens?.length || draft.length >= 32}>
              <Plus size={16} aria-hidden="true" /> Add a screen
            </button>
            <span className={styles.spacer} />
            {dirty && (
              <button className="button" data-variant="quiet" type="button" onClick={() => setDraft(draftOf(saved))}>
                Discard changes
              </button>
            )}
            <button
              className="button"
              data-variant="primary"
              type="button"
              disabled={!dirty || noDays || save.isPending}
              onClick={() => save.mutate()}
            >
              {save.isPending ? "Saving" : "Save playlist"}
            </button>
          </div>
          {noDays && (
            <p className="error-text" role="alert">
              Every entry needs at least one day.
            </p>
          )}
          {save.isError && (
            <p className="error-text" role="alert">
              {messageOf(save.error)}
            </p>
          )}
        </>
      )}
    </section>
  );
}

function EntryRow({
  entry,
  index,
  count,
  screens,
  onChange,
  onMove,
  onRemove,
}: {
  entry: Draft;
  index: number;
  count: number;
  screens: ScreenSummary[];
  onChange: (change: Partial<PlaylistEntry>) => void;
  onMove: (by: number) => void;
  onRemove: () => void;
}) {
  const id = useId();
  const days = entry.days ?? [...WEEK];
  const toggle = (day: Weekday) => {
    const next = days.includes(day) ? days.filter((d) => d !== day) : WEEK.filter((d) => d === day || days.includes(d));
    onChange({ days: next.length === 7 ? undefined : next });
  };
  const known = screens.some((screen) => screen.id === entry.screenId);
  // The window alone, on any one of its days, for the strip.
  const window = { ...entry, days: undefined };

  return (
    <li className={styles.entry}>
      <span className={styles.position} aria-hidden="true">
        {index + 1}
      </span>
      <div className={styles.fields}>
        <label className={styles.screen}>
          <span className="visually-hidden">Screen</span>
          <select
            className="select"
            value={entry.screenId}
            onChange={(event) => onChange({ screenId: Number(event.target.value) })}
          >
            {!known && <option value={entry.screenId}>A deleted screen</option>}
            {screens.map((screen) => (
              <option key={screen.id} value={screen.id}>
                {screen.name}
              </option>
            ))}
          </select>
        </label>
        <fieldset className={styles.days}>
          <legend className="visually-hidden">Days: {daysLabel(entry.days)}</legend>
          {WEEK.map((day) => (
            <button
              key={day}
              type="button"
              className={styles.day}
              aria-pressed={days.includes(day)}
              onClick={() => toggle(day)}
              title={day}
            >
              {shortDay(day).slice(0, 2)}
            </button>
          ))}
        </fieldset>
        <div className={styles.times}>
          <label>
            <span className="visually-hidden">From</span>
            <input
              id={`${id}-from`}
              className="input"
              type="time"
              step={60}
              value={entry.from ?? ""}
              onChange={(event) => onChange({ from: event.target.value || undefined })}
            />
          </label>
          <span className="annotation" aria-hidden="true">
            to
          </span>
          <label>
            <span className="visually-hidden">Until</span>
            <input
              className="input"
              type="time"
              step={60}
              value={entry.until ?? ""}
              onChange={(event) => onChange({ until: event.target.value || undefined })}
            />
          </label>
        </div>
        <div className={styles.strip}>
          <DayStrip day="monday" entries={[{ entry: window, name: "" }]} hours={index === count - 1} label={`${daysLabel(entry.days)}, ${windowLabel(entry)}`} />
          <span className="annotation">
            {daysLabel(entry.days)} · {windowLabel(entry)}
          </span>
        </div>
      </div>
      <div className={styles.tools}>
        <button type="button" className="icon-button" onClick={() => onMove(-1)} disabled={index === 0} aria-label="Move up">
          <ArrowUp size={16} />
        </button>
        <button
          type="button"
          className="icon-button"
          onClick={() => onMove(1)}
          disabled={index === count - 1}
          aria-label="Move down"
        >
          <ArrowDown size={16} />
        </button>
        <button type="button" className="icon-button" onClick={onRemove} aria-label="Remove from playlist">
          <X size={16} />
        </button>
      </div>
    </li>
  );
}
