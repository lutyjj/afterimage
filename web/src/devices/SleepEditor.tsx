import type { Device, ScreenSummary, SleepWindow } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import {
  Actions,
  BAND_CLASS,
  Button,
  Card,
  CardBody,
  CardHead,
  CellSelect,
  classes,
  IconButton,
  Nothing,
  SettingRow,
  SettingToggle,
  SettlingItem,
  SettlingRows,
  useToast,
  useUnsavedChanges,
} from "linen";
import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { FIELD_WIDTH } from "../app/fieldWidth";
import { client, messageOf, queryClient } from "../app/client";
import { DayTrack, HourAxis } from "./DayTrack";
import { HoursFields } from "./HoursFields";
import { hoursLabel, spansOn } from "./schedule";
import { type Draft, draftOf, entryOf, newKey, sameAs } from "./scheduleDraft";
import { deviceQuery, devicesQuery } from "./queries";

/** The window a new or no longer all-day sleep window takes: the night. */
const NIGHT = { from: "23:00", until: "07:00" };

/** At most as many windows as the server keeps. */
const MOST_WINDOWS = 14;

/** When a device sleeps, on the same days and times a schedule slot takes, and
 * what it shows meanwhile. */
export function SleepEditor({
  device,
  screens,
  timeZone,
}: {
  device: Device;
  screens: ScreenSummary[] | undefined;
  timeZone: string | undefined;
}) {
  const saved = device.sleep;
  const [windows, setWindows] = useState(() => draftOf(saved.windows));
  const [screenId, setScreenId] = useState(saved.screenId);
  const dirty = !sameAs(windows, saved.windows) || screenId !== saved.screenId;

  // Sleep saved elsewhere replaces an untouched draft, as the schedule's does.
  const previous = useRef(saved);
  useEffect(() => {
    const before = previous.current;
    previous.current = saved;
    setWindows((current) => (sameAs(current, before.windows) ? draftOf(saved.windows, current) : current));
    setScreenId((current) => (current === before.screenId ? saved.screenId : current));
  }, [saved]);

  const toast = useToast();
  const save = useMutation({
    mutationFn: () => client.setSleep({ deviceId: device.id, sleep: { windows: windows.map(entryOf), screenId } }),
    onSuccess: ({ device: updated }) => {
      queryClient.setQueryData(deviceQuery(updated.id).queryKey, updated);
      void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      setWindows((current) => draftOf(updated.sleep.windows, current));
      setScreenId(updated.sleep.screenId);
      toast.show({ text: "Sleep saved. The display takes it at its next check-in." });
    },
  });

  useUnsavedChanges(dirty, { name: "sleep", pending: save.isPending });

  const update = (key: number, change: Partial<Draft<SleepWindow>>) =>
    setWindows((all) => all.map((window) => (window.key === key ? { ...window, ...change } : window)));
  const remove = (key: number) => setWindows((all) => all.filter((window) => window.key !== key));
  const add = () => setWindows((all) => [...all, { key: newKey(), ...NIGHT, allDay: false }]);
  const noDays = windows.some((window) => window.days?.length === 0);
  // Until the screens have loaded, a saved sleep screen is not taken for deleted.
  const known = screenId === undefined || screens === undefined || screens.some((screen) => screen.id === screenId);

  return (
    <Card>
      <CardHead
        title="Sleep"
        description={
          <>
            In these hours the display sleeps: it shows its sleep screen and checks in again when they end, which saves
            its battery. A change reaches a sleeping display when it wakes.
            {timeZone && <> Times are in {timeZone}.</>}
          </>
        }
      >
        <Button onClick={add} disabled={windows.length >= MOST_WINDOWS}>
          <Plus aria-hidden="true" className="size-4" /> Add a sleep window
        </Button>
      </CardHead>
      <HoldToggle device={device} />
      <SettingRow
        title="While asleep, it shows"
        description="The server's own says when it wakes. A screen that cannot be drawn gives way to it."
      >
        <CellSelect
          aria-label="While asleep, it shows"
          width={FIELD_WIDTH}
          placeholder="The time it wakes"
          choices={[
            ...(known ? [] : [{ value: String(screenId), label: "A deleted screen" }]),
            ...(screens ?? []).map((screen) => ({ value: String(screen.id), label: screen.name })),
          ]}
          value={screenId === undefined ? "" : String(screenId)}
          onValueChange={(value) => setScreenId(value === "" ? undefined : Number(value))}
        />
      </SettingRow>
      {windows.length === 0 && <Nothing>No sleep windows: awake around the clock.</Nothing>}
      {/* The list stays on the page while it is empty, only hidden, so the
          first window added to it arrives into the same list. */}
      <div role="list" aria-label="Sleep windows" hidden={windows.length === 0}>
        <SettlingRows scope={`sleep-${device.id}`} marksArrivals>
          {windows.map((window) => (
            <WindowRow
              key={window.key}
              window={window}
              onChange={(change) => update(window.key, change)}
              onRemove={() => remove(window.key)}
            />
          ))}
        </SettlingRows>
      </div>
      <CardBody>
        {noDays && (
          <p role="alert" className="text-sm text-danger">
            Every sleep window needs at least one day.
          </p>
        )}
        {save.isError && (
          <p role="alert" className="text-sm text-danger">
            {messageOf(save.error)}
          </p>
        )}
        <Actions className="justify-end">
          {dirty && (
            <Button
              onClick={() => {
                // The rows it keeps stay the same rows, not new arrivals.
                setWindows((current) => draftOf(saved.windows, current));
                setScreenId(saved.screenId);
              }}
            >
              Discard changes
            </Button>
          )}
          <Button variant="primary" disabled={!dirty || noDays || save.isPending} onClick={() => save.mutate()}>
            {save.isPending ? "Saving" : "Save sleep"}
          </Button>
        </Actions>
      </CardBody>
    </Card>
  );
}

function WindowRow({
  window,
  onChange,
  onRemove,
}: {
  window: Draft<SleepWindow>;
  onChange: (change: Partial<Draft<SleepWindow>>) => void;
  onRemove: () => void;
}) {
  const saved = entryOf(window);
  return (
    <SettlingItem role="listitem" className={classes(BAND_CLASS, "grid gap-y-2")}>
      {/* On a phone the days and the remove button share the first line and
          the hours take the next; wider, all of them share the line while it
          holds them and the remove button ends it. */}
      <div className="flex min-w-0 flex-wrap items-start gap-2">
        <HoursFields
          hours={window}
          timed={NIGHT}
          daysClassName="max-w-fit flex-1 lg:flex-none"
          timesClassName="order-2 basis-full lg:order-none lg:basis-auto"
          onChange={onChange}
        />
        <IconButton
          aria-label="Remove sleep window"
          title="Remove sleep window"
          className="order-1 ml-auto lg:order-last"
          onClick={onRemove}
        >
          <X aria-hidden="true" className="size-4" />
        </IconButton>
      </div>
      <div className="grid gap-1">
        {/* The window alone, on any one of its days. */}
        <DayTrack
          marks={spansOn({ ...saved, days: undefined }, "monday").map(([start, end]) => ({
            start,
            end,
            label: "Asleep",
            tone: "asleep",
          }))}
          label={hoursLabel(saved)}
        />
        <HourAxis />
        <span className="text-xs text-text-muted">{hoursLabel(saved)}</span>
      </div>
    </SettlingItem>
  );
}

/** Holds the display asleep outside its windows until let go, as Home
 * Assistant's switch does. It saves as it turns. */
function HoldToggle({ device }: { device: Device }) {
  const toast = useToast();
  const hold = useMutation({
    mutationFn: (held: boolean) => client.updateDevice({ deviceId: device.id, sleepHeld: held }),
    onSuccess: ({ device: updated }) => {
      queryClient.setQueryData(deviceQuery(updated.id).queryKey, updated);
      void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
    },
    onError: (error) => toast.show({ text: `Not changed. ${messageOf(error)}`, sticky: true }),
  });
  return (
    <SettingToggle
      title="Asleep now"
      description="Holds the display asleep outside its windows until turned off. It falls asleep at its next check-in, and learns it may wake at its next hourly one."
      label="Asleep now"
      checked={hold.isPending ? (hold.variables ?? device.sleepHeld) : device.sleepHeld}
      disabled={hold.isPending}
      onCheckedChange={(held) => hold.mutate(held)}
    />
  );
}
