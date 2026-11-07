import type { Device, ScreenSummary, SleepWindow } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import {
  Actions,
  Button,
  Card,
  CardBody,
  CardHead,
  IconButton,
  Select,
  SettlingItem,
  SettlingRows,
  useToast,
} from "linen";
import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { client, messageOf, queryClient } from "../app/client";
import { DayStrip } from "./DayStrip";
import { HoursFields } from "./HoursFields";
import { daysLabel, windowLabel } from "./schedule";
import { type Draft, draftOf, entryOf, newKey, sameAs } from "./scheduleDraft";
import { deviceQuery, devicesQuery } from "./queries";

/** The window a new or no longer all-day sleep window takes: the night. */
const NIGHT = { from: "23:00", until: "07:00" };

/** At most as many windows as the server keeps. */
const MOST_WINDOWS = 14;

/** When a device sleeps, on the same days and times a playlist entry takes, and
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

  // Sleep saved elsewhere replaces an untouched draft, as the playlist's does.
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

  const update = (key: number, change: Partial<Draft<SleepWindow>>) =>
    setWindows((all) => all.map((window) => (window.key === key ? { ...window, ...change } : window)));
  const remove = (key: number) => setWindows((all) => all.filter((window) => window.key !== key));
  const add = () => setWindows((all) => [...all, { key: newKey(), ...NIGHT, allDay: false }]);
  const noDays = windows.some((window) => window.days?.length === 0);
  const known = screenId === undefined || screens?.some((screen) => screen.id === screenId);

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
      />
      <CardBody className="grid gap-4">
        <div className="max-w-md">
          <Select
            label="While asleep, it shows"
            hint="The server's own says when it wakes. A screen that cannot be drawn gives way to it."
            placeholder="The time it wakes"
            choices={[
              ...(known ? [] : [{ value: String(screenId), label: "A deleted screen" }]),
              ...(screens ?? []).map((screen) => ({ value: String(screen.id), label: screen.name })),
            ]}
            value={screenId === undefined ? "" : String(screenId)}
            onValueChange={(value) => setScreenId(value === "" ? undefined : Number(value))}
          />
        </div>
        {windows.length === 0 && <p className="max-w-2xl text-sm text-text-secondary">Awake around the clock.</p>}
        {/* The list stays on the page while it is empty, only hidden, so the
            first window added to it arrives into the same list. */}
        <div
          role="list"
          hidden={windows.length === 0}
          className="grid overflow-hidden rounded-button border border-border"
        >
          <SettlingRows scope={`sleep-${device.id}`} marksArrivals>
            {windows.map((window, index) => (
              <WindowRow
                key={window.key}
                window={window}
                last={index === windows.length - 1}
                onChange={(change) => update(window.key, change)}
                onRemove={() => remove(window.key)}
              />
            ))}
          </SettlingRows>
        </div>
        <Actions>
          <Button onClick={add} disabled={windows.length >= MOST_WINDOWS}>
            <Plus aria-hidden="true" className="size-4" /> Add a sleep window
          </Button>
          <span className="flex-1" />
          {dirty && (
            <Button
              onClick={() => {
                setWindows(draftOf(saved.windows));
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
      </CardBody>
    </Card>
  );
}

function WindowRow({
  window,
  last,
  onChange,
  onRemove,
}: {
  window: Draft<SleepWindow>;
  last: boolean;
  onChange: (change: Partial<Draft<SleepWindow>>) => void;
  onRemove: () => void;
}) {
  const saved = entryOf(window);
  return (
    <SettlingItem
      role="listitem"
      className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-x-3 gap-y-2 border-t border-border bg-surface p-3 first:border-t-0"
    >
      <div className="flex flex-wrap items-start gap-3">
        <HoursFields hours={window} timed={NIGHT} onChange={onChange} />
      </div>
      <IconButton aria-label="Remove sleep window" title="Remove sleep window" onClick={onRemove}>
        <X aria-hidden="true" className="size-4" />
      </IconButton>
      <div className="col-span-2 grid gap-1">
        <DayStrip
          day="monday"
          lanes={[{ hours: [{ ...saved, days: undefined }], name: "" }]}
          hours={last}
          names={false}
          label={`${daysLabel(saved.days)}, ${windowLabel(saved)}`}
        />
        <span className="text-xs text-text-muted">
          {daysLabel(saved.days)}, {windowLabel(saved)}
        </span>
      </div>
    </SettlingItem>
  );
}
