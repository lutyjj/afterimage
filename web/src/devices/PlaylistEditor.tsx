import type { Device, PlaylistEntry, ScreenSummary } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import {
  Actions,
  Button,
  Card,
  CardBody,
  CardHead,
  CellSelect,
  IconButton,
  LINK_CLASS,
  SettlingItem,
  SettlingRows,
  useToast,
} from "linen";
import { ArrowDown, ArrowUp, Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import { DayStrip, dayLanes } from "./DayStrip";
import { HoursFields } from "./HoursFields";
import { draftOf, entryOf, newKey, sameAs, type Draft } from "./scheduleDraft";
import { daysLabel, nowIn, windowLabel } from "./schedule";
import { deviceQuery, devicesQuery } from "./queries";

/** The window an entry takes when it stops being all day, as a start to edit. */
const DAYTIME = { from: "08:00", until: "20:00" };

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
  const dirty = !sameAs(draft, saved);

  // A playlist saved elsewhere replaces an untouched draft. The query keeps
  // `saved` the same array while its content is unchanged (structural sharing).
  const previous = useRef(saved);
  useEffect(() => {
    const before = previous.current;
    previous.current = saved;
    setDraft((current) => (sameAs(current, before) ? draftOf(saved, current) : current));
  }, [saved]);

  const toast = useToast();
  const save = useMutation({
    mutationFn: () => client.setPlaylist({ deviceId: device.id, playlist: draft.map(entryOf) }),
    onSuccess: ({ device: updated }) => {
      queryClient.setQueryData(deviceQuery(updated.id).queryKey, updated);
      void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      setDraft((current) => draftOf(updated.playlist, current));
      toast.show({ text: "Playlist saved. The display takes it at its next check-in." });
    },
  });

  const update = (key: number, change: Partial<Draft<PlaylistEntry>>) =>
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
    if (screen) setDraft((entries) => [...entries, { key: newKey(), screenId: screen.id, allDay: true }]);
  };

  const name = (id: number) => screens?.find((screen) => screen.id === id)?.name ?? "A deleted screen";
  const now = timeZone ? nowIn(timeZone) : undefined;
  const noDays = draft.some((entry) => entry.days?.length === 0);

  return (
    <Card>
      <CardHead
        title="Playlist"
        description={
          <>
            At each check-in the display shows the next screen that is current, in this order.
            {timeZone && <> Times are in {timeZone}.</>}
          </>
        }
      />
      <CardBody className="grid gap-4">
        {now && (
          <div className="grid gap-2">
            <span className="text-sm font-medium text-text-secondary">Today</span>
            <DayStrip
              day={now.day}
              now={now.minute}
              lanes={dayLanes(
                draft.map((entry) => ({ hours: entryOf(entry), name: name(entry.screenId) })),
                device.sleep.windows,
              )}
              label="Today's playlist and sleep"
            />
          </div>
        )}

        {screens?.length === 0 ? (
          <p className="max-w-2xl text-sm text-text-secondary">
            No screens yet.{" "}
            <Link to="/screens/new" className={LINK_CLASS}>
              Write the first screen
            </Link>
            , then come back to give it a place here.
          </p>
        ) : (
          <>
            {draft.length === 0 && (
              <p className="max-w-2xl text-sm text-text-secondary">
                Nothing scheduled: the display shows “Nothing to show”. Add a screen to give it something.
              </p>
            )}
            {/* The list stays on the page while it is empty, only hidden, so
                the first screen added to it arrives into the same list. */}
            <div
              role="list"
              hidden={draft.length === 0}
              className="grid overflow-hidden rounded-button border border-border"
            >
              <SettlingRows scope={`playlist-${device.id}`} marksArrivals>
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
              </SettlingRows>
            </div>
            <Actions>
              <Button onClick={add} disabled={!screens?.length || draft.length >= 32}>
                <Plus aria-hidden="true" className="size-4" /> Add a screen
              </Button>
              <span className="flex-1" />
              {dirty && <Button onClick={() => setDraft(draftOf(saved))}>Discard changes</Button>}
              <Button variant="primary" disabled={!dirty || noDays || save.isPending} onClick={() => save.mutate()}>
                {save.isPending ? "Saving" : "Save playlist"}
              </Button>
            </Actions>
            {noDays && (
              <p role="alert" className="text-sm text-danger">
                Every entry needs at least one day.
              </p>
            )}
            {save.isError && (
              <p role="alert" className="text-sm text-danger">
                {messageOf(save.error)}
              </p>
            )}
          </>
        )}
      </CardBody>
    </Card>
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
  entry: Draft<PlaylistEntry>;
  index: number;
  count: number;
  screens: ScreenSummary[];
  onChange: (change: Partial<Draft<PlaylistEntry>>) => void;
  onMove: (by: number) => void;
  onRemove: () => void;
}) {
  const known = screens.some((screen) => screen.id === entry.screenId);
  // The window alone, on any one of its days, for the strip.
  const window = { ...entryOf(entry), days: undefined };

  const choices = [
    ...(known ? [] : [{ value: String(entry.screenId), label: "A deleted screen" }]),
    ...screens.map((screen) => ({ value: String(screen.id), label: screen.name })),
  ];

  return (
    <SettlingItem
      role="listitem"
      className="grid grid-cols-[1.25rem_minmax(0,1fr)] items-start gap-x-3 gap-y-2 border-t border-border bg-surface p-3 first:border-t-0 sm:grid-cols-[1.5rem_minmax(0,1fr)_auto]"
    >
      <span
        aria-hidden="true"
        className="grid h-(--height-control) place-items-center text-sm font-semibold text-text-muted tabular-nums"
      >
        {index + 1}
      </span>
      <div className="grid items-start gap-3 lg:grid-cols-[minmax(10rem,16rem)_minmax(0,1fr)] 2xl:grid-cols-[minmax(10rem,14rem)_auto_minmax(0,1fr)]">
        <CellSelect
          aria-label="Screen"
          className="w-full font-medium"
          choices={choices}
          value={String(entry.screenId)}
          onValueChange={(value) => onChange({ screenId: Number(value) })}
        />
        <HoursFields hours={entry} timed={DAYTIME} className="lg:col-span-2 2xl:col-span-1" onChange={onChange} />
      </div>
      <div className="col-start-2 flex gap-1 sm:col-start-auto">
        <IconButton aria-label="Move up" title="Move up" onClick={() => onMove(-1)} disabled={index === 0}>
          <ArrowUp aria-hidden="true" className="size-4" />
        </IconButton>
        <IconButton aria-label="Move down" title="Move down" onClick={() => onMove(1)} disabled={index === count - 1}>
          <ArrowDown aria-hidden="true" className="size-4" />
        </IconButton>
        <IconButton aria-label="Remove from playlist" title="Remove from playlist" onClick={onRemove}>
          <X aria-hidden="true" className="size-4" />
        </IconButton>
      </div>
      <div className="col-[2/-1] grid gap-1">
        <DayStrip
          day="monday"
          lanes={[{ hours: [window], name: "" }]}
          hours={index === count - 1}
          names={false}
          label={`${daysLabel(entry.days)}, ${windowLabel(entryOf(entry))}`}
        />
        <span className="text-xs text-text-muted">
          {daysLabel(entry.days)}, {windowLabel(entryOf(entry))}
        </span>
      </div>
    </SettlingItem>
  );
}
