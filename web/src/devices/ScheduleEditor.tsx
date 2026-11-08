import type { Device, Playlist, ScheduleSlot, ScreenSummary, Shows, Weekday } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import {
  Actions,
  Button,
  Card,
  CardBody,
  CardHead,
  CellSelect,
  type Choice,
  IconButton,
  LINK_CLASS,
  Select,
  SettingRow,
  SettlingItem,
  SettlingRows,
  classes,
  useToast,
  useUnsavedChanges,
} from "linen";
import { Plus, X } from "lucide-react";
import { Fragment, useEffect, useId, useMemo, useRef, useState } from "react";
import { Link } from "react-router";

import { client, messageOf, queryClient } from "../app/client";
import { timeOfDay } from "../app/format";
import { DayTrack, HourAxis, marksOf, type Names, namesOf } from "./DayTrack";
import { HoursFields } from "./HoursFields";
import { pinOf, pinValue } from "./pin";
import { deviceQuery, devicesQuery } from "./queries";
import { dayOf, nowIn, overlapOf, type Plan, shortDay, WEEK } from "./schedule";
import { type Draft, draftOf, entryOf, newKey, sameAs } from "./scheduleDraft";

/** The window a slot takes when it stops being all day, as a start to edit. */
const DAYTIME = { from: "08:00", until: "20:00" };

/** As many slots as the server keeps. */
const MOST_SLOTS = 32;

const DAY_NAMES: Record<Weekday, string> = {
  monday: "Monday",
  tuesday: "Tuesday",
  wednesday: "Wednesday",
  thursday: "Thursday",
  friday: "Friday",
  saturday: "Saturday",
  sunday: "Sunday",
};

/** Which playlist the display shows when it is awake: the one it usually
 * shows, and the days and hours another takes its place, which never overlap.
 * Above them, a playlist or screen pinned in place of the schedule. */
export function ScheduleEditor({
  device,
  playlists,
  screens,
  timeZone,
}: {
  device: Device;
  playlists: Playlist[] | undefined;
  screens: ScreenSummary[] | undefined;
  timeZone: string | undefined;
}) {
  const saved = device.schedule;
  const [usual, setUsual] = useState(saved.playlistId);
  const [slots, setSlots] = useState(() => draftOf(saved.slots));
  const dirty = usual !== saved.playlistId || !sameAs(slots, saved.slots);

  // A schedule saved elsewhere replaces an untouched draft. The query keeps
  // `saved` the same object while its content is unchanged (structural sharing).
  const previous = useRef(saved);
  useEffect(() => {
    const before = previous.current;
    previous.current = saved;
    setSlots((current) => (sameAs(current, before.slots) ? draftOf(saved.slots, current) : current));
    setUsual((current) => (current === before.playlistId ? saved.playlistId : current));
  }, [saved]);

  const toast = useToast();
  const save = useMutation({
    mutationFn: () =>
      client.setSchedule({ deviceId: device.id, schedule: { playlistId: usual, slots: slots.map(entryOf) } }),
    onSuccess: ({ device: updated }) => {
      queryClient.setQueryData(deviceQuery(updated.id).queryKey, updated);
      void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      setSlots((current) => draftOf(updated.schedule.slots, current));
      setUsual(updated.schedule.playlistId);
      toast.show({ text: "Schedule saved. The display takes it at its next check-in." });
    },
  });

  useUnsavedChanges(dirty, { name: "the schedule", pending: save.isPending });

  const update = (key: number, change: Partial<Draft<ScheduleSlot>>) =>
    setSlots((all) => all.map((slot) => (slot.key === key ? { ...slot, ...change } : slot)));
  const remove = (key: number) => setSlots((all) => all.filter((slot) => slot.key !== key));
  const add = () => {
    // A playlist the schedule does not show yet, as the likeliest start.
    const named = new Set([usual, ...slots.map((slot) => slot.playlistId)]);
    const playlist = playlists?.find((candidate) => !named.has(candidate.id)) ?? playlists?.[0];
    if (playlist) setSlots((all) => [...all, { key: newKey(), playlistId: playlist.id, ...DAYTIME, allDay: false }]);
  };

  const drafted = useMemo(() => slots.map(entryOf), [slots]);
  const windows = device.sleep.windows;
  const plan = useMemo(() => ({ schedule: { playlistId: usual, slots: drafted }, windows }), [usual, drafted, windows]);
  const overlap = useMemo(() => overlapOf(drafted), [drafted]);
  const noDays = slots.some((slot) => slot.days?.length === 0);
  const names = namesOf(playlists, screens);
  const playlistChoices = (playlists ?? []).map((playlist) => ({ value: String(playlist.id), label: playlist.name }));

  return (
    <Card>
      <CardHead
        title="Schedule"
        description={
          <>
            Which playlist the display shows while it is awake.
            {timeZone && <> Times are in {timeZone}.</>}
          </>
        }
      />
      {(playlists?.length ?? 0) > 0 || (screens?.length ?? 0) > 0 ? (
        <PinRow device={device} playlists={playlists ?? []} screens={screens ?? []} names={names} />
      ) : null}
      <CardBody className="grid gap-5">
        {playlists?.length === 0 ? (
          <p className="max-w-2xl text-sm text-text-secondary">
            No playlists yet.{" "}
            <Link to="/playlists/new" className={LINK_CLASS}>
              Make the first playlist
            </Link>
            , then come back to give it a place here.
          </p>
        ) : (
          <>
            <Week plan={plan} names={names} timeZone={timeZone} />
            <div className="max-w-md">
              <Select
                label="Usually shows"
                hint="Whenever no time below is current."
                placeholder="Nothing"
                choices={[
                  ...(usual === undefined || playlists === undefined || playlists.some((p) => p.id === usual)
                    ? []
                    : [{ value: String(usual), label: "A deleted playlist" }]),
                  ...playlistChoices,
                ]}
                value={usual === undefined ? "" : String(usual)}
                onValueChange={(value) => setUsual(value === "" ? undefined : Number(value))}
              />
            </div>
            <section aria-labelledby={`slots-${device.id}`} className="grid gap-2">
              <h3 id={`slots-${device.id}`} className="text-sm font-medium">
                At these times instead
              </h3>
              {slots.length === 0 && (
                <p className="max-w-2xl text-sm text-text-secondary">
                  None: the display shows its usual playlist all week.
                </p>
              )}
              {/* The list stays on the page while it is empty, only hidden, so
                  the first slot added to it arrives into the same list. */}
              <div
                role="list"
                hidden={slots.length === 0}
                className="grid overflow-hidden rounded-button border border-border"
              >
                <SettlingRows scope={`schedule-${device.id}`} marksArrivals>
                  {slots.map((slot, index) => (
                    <SlotRow
                      key={slot.key}
                      slot={slot}
                      index={index}
                      choices={playlistChoices}
                      known={playlists === undefined || playlists.some((p) => p.id === slot.playlistId)}
                      overlaps={
                        overlap && overlap.slots[1] === index
                          ? `Overlaps time ${overlap.slots[0] + 1} on ${DAY_NAMES[overlap.day]} at ${timeOfDay(overlap.minute)}. A display shows one playlist at a time.`
                          : undefined
                      }
                      onChange={(change) => update(slot.key, change)}
                      onRemove={() => remove(slot.key)}
                    />
                  ))}
                </SettlingRows>
              </div>
            </section>
            <Actions>
              <Button onClick={add} disabled={!playlists?.length || slots.length >= MOST_SLOTS}>
                <Plus aria-hidden="true" className="size-4" /> Add a time
              </Button>
              <span className="flex-1" />
              {dirty && (
                <Button
                  onClick={() => {
                    // The rows it keeps stay the same rows, not new arrivals.
                    setSlots((current) => draftOf(saved.slots, current));
                    setUsual(saved.playlistId);
                  }}
                >
                  Discard changes
                </Button>
              )}
              <Button
                variant="primary"
                disabled={!dirty || noDays || overlap !== undefined || save.isPending}
                onClick={() => save.mutate()}
              >
                {save.isPending ? "Saving" : "Save schedule"}
              </Button>
            </Actions>
            {noDays && (
              <p role="alert" className="text-sm text-danger">
                Every time needs at least one day.
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

function SlotRow({
  slot,
  index,
  choices,
  known,
  overlaps,
  onChange,
  onRemove,
}: {
  slot: Draft<ScheduleSlot>;
  index: number;
  choices: readonly Choice[];
  known: boolean;
  overlaps: string | undefined;
  onChange: (change: Partial<Draft<ScheduleSlot>>) => void;
  onRemove: () => void;
}) {
  const problem = useId();
  return (
    <SettlingItem
      role="listitem"
      aria-describedby={overlaps ? problem : undefined}
      className="grid grid-cols-[1.25rem_minmax(0,1fr)_auto] items-start gap-x-3 gap-y-2 border-t border-border bg-surface p-3 first:border-t-0 sm:grid-cols-[1.5rem_minmax(0,1fr)_auto]"
    >
      <span
        aria-hidden="true"
        className={classes(
          "grid h-(--height-control) place-items-center text-sm font-semibold tabular-nums",
          overlaps ? "text-danger" : "text-text-muted",
        )}
      >
        {index + 1}
      </span>
      <div className="grid items-start gap-3 lg:grid-cols-[minmax(10rem,16rem)_minmax(0,1fr)] 2xl:grid-cols-[minmax(10rem,14rem)_auto_minmax(0,1fr)]">
        <CellSelect
          aria-label={`Playlist of time ${index + 1}`}
          className="w-full font-medium"
          invalid={!known}
          choices={[...(known ? [] : [{ value: String(slot.playlistId), label: "A deleted playlist" }]), ...choices]}
          value={String(slot.playlistId)}
          onValueChange={(value) => onChange({ playlistId: Number(value) })}
        />
        <HoursFields hours={slot} timed={DAYTIME} className="lg:col-span-2 2xl:col-span-1" onChange={onChange} />
      </div>
      <IconButton aria-label={`Remove time ${index + 1}`} title="Remove" onClick={onRemove}>
        <X aria-hidden="true" className="size-4" />
      </IconButton>
      {overlaps && (
        <p id={problem} className="col-[2/-1] text-sm text-danger">
          {overlaps}
        </p>
      )}
    </SettlingItem>
  );
}

/** A week of what the display shows, a track a day, today's marked at now.
 * It draws the schedule being edited over the saved sleep. */
function Week({ plan, names, timeZone }: { plan: Plan; names: Names; timeZone: string | undefined }) {
  const now = timeZone ? nowIn(timeZone) : undefined;
  // Laid out again only when the plan changes, not at each keystroke elsewhere.
  const days = useMemo(() => WEEK.map((day) => [day, dayOf(plan, day)] as const), [plan]);
  return (
    <div role="group" aria-label="Each week" className="grid gap-2">
      <span aria-hidden="true" className="text-sm font-medium text-text-secondary">
        Each week
      </span>
      <div className="grid grid-cols-[2.25rem_minmax(0,1fr)] items-center gap-x-3 gap-y-1.5">
        {days.map(([day, stretches]) => {
          const today = day === now?.day;
          return (
            <Fragment key={day}>
              <span
                aria-hidden="true"
                className={classes("text-xs", today ? "font-semibold text-text" : "text-text-muted")}
              >
                {shortDay(day)}
              </span>
              <DayTrack
                marks={marksOf(stretches, names)}
                now={today ? now?.minute : undefined}
                label={today ? `${DAY_NAMES[day]}, today` : DAY_NAMES[day]}
              />
            </Fragment>
          );
        })}
        <span />
        <HourAxis />
      </div>
    </div>
  );
}

/** What the display shows while awake: its schedule, or one playlist or screen
 * pinned in its place until set back. It saves as it changes. */
function PinRow({
  device,
  playlists,
  screens,
  names,
}: {
  device: Device;
  playlists: readonly Playlist[];
  screens: readonly ScreenSummary[];
  names: Names;
}) {
  const toast = useToast();
  const pin = useMutation({
    mutationFn: (shows: Shows) => client.updateDevice({ deviceId: device.id, shows }),
    onSuccess: ({ device: updated }) => {
      queryClient.setQueryData(deviceQuery(updated.id).queryKey, updated);
      void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      toast.show({
        text:
          updated.shows.schedule !== undefined
            ? "Back to its schedule at its next check-in."
            : "Pinned. The display shows it from its next check-in.",
      });
    },
    onError: (error) => toast.show({ text: `Not changed. ${messageOf(error)}`, sticky: true }),
  });
  const { playlistId, screenId } = device.shows;
  const value = pinValue(device.shows);
  const lost =
    (playlistId !== undefined && !playlists.some((playlist) => playlist.id === playlistId)) ||
    (screenId !== undefined && !screens.some((screen) => screen.id === screenId));
  // A playlist and a screen may share a name, so the kind is named too.
  const pinned =
    playlistId !== undefined
      ? `the playlist ${names.playlist(playlistId)}`
      : screenId !== undefined
        ? `the screen ${names.screen(screenId)}`
        : undefined;
  return (
    <SettingRow
      title="Showing"
      description={
        pinned === undefined
          ? "Pin a playlist or a screen to show it in place of the schedule, until set back."
          : `Pinned to ${pinned}. The schedule waits until this is set back to it.`
      }
    >
      <CellSelect
        aria-label="Showing"
        className="w-64 max-w-full"
        placeholder="Its schedule"
        disabled={pin.isPending}
        choices={[
          ...(lost ? [{ value, label: "A deleted one" }] : []),
          ...(playlists.length > 0
            ? [
                {
                  label: "Playlists",
                  choices: playlists.map((playlist) => ({
                    value: pinValue({ playlistId: playlist.id }),
                    label: playlist.name,
                  })),
                },
              ]
            : []),
          ...(screens.length > 0
            ? [
                {
                  label: "Screens",
                  choices: screens.map((screen) => ({ value: pinValue({ screenId: screen.id }), label: screen.name })),
                },
              ]
            : []),
        ]}
        value={value}
        onValueChange={(chosen) => {
          pin.mutate(pinOf(chosen));
        }}
      />
    </SettingRow>
  );
}
