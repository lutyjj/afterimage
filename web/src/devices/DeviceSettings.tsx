import type { Device, DisplayModel } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import { Actions, Button, Card, CardBody, CardHead, Select, TextField, useToast } from "linen";
import { useEffect, useRef, useState, type FormEvent } from "react";

import { client, messageOf, queryClient } from "../app/client";
import { minutesLabel } from "../app/format";
import { deviceQuery, devicesQuery } from "./queries";

const REFRESH_CHOICES = [5, 10, 15, 30, 60, 120, 240, 720, 1440];

/** Name, display and how often the device wakes, saved together. */
export function DeviceSettings({ device, models }: { device: Device; models: DisplayModel[] | undefined }) {
  const [name, setName] = useState(device.name);
  const [model, setModel] = useState(device.display.model);
  const [palette, setPalette] = useState(device.display.palette);
  const [refresh, setRefresh] = useState(device.refreshMinutes);
  const toast = useToast();

  // Settings saved elsewhere replace the fields this form has not touched, as the
  // schedule does.
  const stored = {
    name: device.name,
    model: device.display.model,
    palette: device.display.palette,
    refresh: device.refreshMinutes,
  };
  const previous = useRef(stored);
  useEffect(() => {
    const before = previous.current;
    previous.current = stored;
    setName((current) => (current.trim() === before.name ? stored.name : current));
    setModel((current) => (current === before.model ? stored.model : current));
    setPalette((current) => (current === before.palette ? stored.palette : current));
    setRefresh((current) => (current === before.refresh ? stored.refresh : current));
  }, [stored.name, stored.model, stored.palette, stored.refresh]);

  const sorted = [...(models ?? [])].sort((a, b) => a.label.localeCompare(b.label));
  const chosen = sorted.find((candidate) => candidate.name === model);
  const palettes = chosen?.palettes ?? [];
  const displayChanged = model !== device.display.model || palette !== device.display.palette;
  const changed = name.trim() !== device.name || displayChanged || refresh !== device.refreshMinutes;
  const refreshChoices = REFRESH_CHOICES.includes(device.refreshMinutes)
    ? REFRESH_CHOICES
    : [...REFRESH_CHOICES, device.refreshMinutes].sort((a, b) => a - b);

  const save = useMutation({
    mutationFn: () =>
      client.updateDevice({
        deviceId: device.id,
        name: name.trim() !== device.name ? name.trim() : undefined,
        display: displayChanged ? { model, palette } : undefined,
        refreshMinutes: refresh !== device.refreshMinutes ? refresh : undefined,
      }),
    onSuccess: ({ device: updated }) => {
      queryClient.setQueryData(deviceQuery(updated.id).queryKey, updated);
      void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
      toast.show({ text: `${updated.name} saved. The display takes it at its next check-in.` });
    },
  });

  const submit = (event: FormEvent) => {
    event.preventDefault();
    save.mutate();
  };

  const modelChoices = [
    ...(chosen ? [] : [{ value: model, label: model }]),
    ...sorted.map((candidate) => ({
      value: candidate.name,
      label: `${candidate.label} · ${candidate.width} × ${candidate.height}`,
    })),
  ];
  const paletteChoices =
    palettes.length === 0
      ? [{ value: palette, label: palette }]
      : palettes.map((candidate) => ({ value: candidate.id, label: candidate.name }));

  return (
    <Card>
      <CardHead title="Name and display" />
      <CardBody>
        <form onSubmit={submit} className="grid max-w-2xl gap-x-4 sm:grid-cols-2">
          <TextField
            label="Name"
            value={name}
            required
            maxLength={64}
            onChange={(event) => setName(event.target.value)}
          />
          <Select
            label="Display"
            choices={modelChoices}
            value={model}
            onValueChange={(value) => {
              const next = sorted.find((candidate) => candidate.name === value);
              setModel(value);
              if (next && !next.palettes.some((candidate) => candidate.id === palette)) {
                setPalette(next.palettes[0]?.id ?? palette);
              }
            }}
          />
          <Select label="Colors" choices={paletteChoices} value={palette} onValueChange={setPalette} />
          <Select
            label="Wakes at least every"
            hint="Sooner when its schedule calls for another playlist. Waking more often drains the battery faster."
            choices={refreshChoices.map((minutes) => ({ value: String(minutes), label: minutesLabel(minutes) }))}
            value={String(refresh)}
            onValueChange={(value) => setRefresh(Number(value))}
          />
          <Actions className="sm:col-span-2">
            <Button type="submit" variant="primary" disabled={!changed || save.isPending}>
              {save.isPending ? "Saving" : "Save"}
            </Button>
            {save.isError && (
              <span role="alert" className="text-sm text-danger">
                {messageOf(save.error)}
              </span>
            )}
          </Actions>
        </form>
      </CardBody>
    </Card>
  );
}
