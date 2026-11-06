import type { Device, DisplayModel } from "@afterimage/management-client";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";

import { client, messageOf, queryClient } from "../app/client";
import { minutesLabel } from "../app/format";
import styles from "./DeviceSettings.module.css";
import { deviceQuery, devicesQuery } from "./queries";

const REFRESH_CHOICES = [5, 10, 15, 30, 60, 120, 240, 720, 1440];

/** Name, display and how often the device wakes, saved together. */
export function DeviceSettings({ device, models }: { device: Device; models: DisplayModel[] | undefined }) {
  const [name, setName] = useState(device.name);
  const [model, setModel] = useState(device.display.model);
  const [palette, setPalette] = useState(device.display.palette);
  const [refresh, setRefresh] = useState(device.refreshMinutes);
  const [saved, setSaved] = useState(false);

  // Settings saved elsewhere replace the fields this form has not touched, as the
  // playlist does.
  const stored = { name: device.name, model: device.display.model, palette: device.display.palette, refresh: device.refreshMinutes };
  const previous = useRef(stored);
  useEffect(() => {
    const before = previous.current;
    previous.current = stored;
    setName((current) => (current.trim() === before.name ? stored.name : current));
    setModel((current) => (current === before.model ? stored.model : current));
    setPalette((current) => (current === before.palette ? stored.palette : current));
    setRefresh((current) => (current === before.refresh ? stored.refresh : current));
    // The stored values are what change; the object is new on every render.
  }, [JSON.stringify(stored)]);

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
      setSaved(true);
    },
  });

  const submit = (event: FormEvent) => {
    event.preventDefault();
    save.mutate();
  };

  return (
    <section className={styles.section} aria-labelledby="settings-title">
      <h2 id="settings-title" className={styles.title}>
        Settings
      </h2>
      <form className={styles.form} onSubmit={submit} onChange={() => setSaved(false)}>
        <label className="field">
          <span>Name</span>
          <input
            className="input"
            value={name}
            required
            maxLength={64}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="field">
          <span>Display</span>
          <select
            className="select"
            value={model}
            onChange={(event) => {
              const next = sorted.find((candidate) => candidate.name === event.target.value);
              setModel(event.target.value);
              if (next && !next.palettes.some((candidate) => candidate.id === palette)) {
                setPalette(next.palettes[0]?.id ?? palette);
              }
            }}
          >
            {!chosen && <option value={model}>{model}</option>}
            {sorted.map((candidate) => (
              <option key={candidate.name} value={candidate.name}>
                {candidate.label} · {candidate.width} × {candidate.height}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Colors</span>
          <select className="select" value={palette} onChange={(event) => setPalette(event.target.value)}>
            {palettes.length === 0 && <option value={palette}>{palette}</option>}
            {palettes.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Wakes at least every</span>
          <select className="select" value={refresh} onChange={(event) => setRefresh(Number(event.target.value))}>
            {refreshChoices.map((minutes) => (
              <option key={minutes} value={minutes}>
                {minutesLabel(minutes)}
              </option>
            ))}
          </select>
          <span className="hint">
            Sooner when its playlist changes. Waking more often drains the battery faster.
          </span>
        </label>
        <div className={styles.actions}>
          <button className="button" data-variant="primary" type="submit" disabled={!changed || save.isPending}>
            {save.isPending ? "Saving" : "Save settings"}
          </button>
          {saved && !changed && (
            <span className="annotation" role="status">
              Saved. The display takes them at its next check-in.
            </span>
          )}
          {save.isError && (
            <span className="error-text" role="alert">
              {messageOf(save.error)}
            </span>
          )}
        </div>
      </form>
    </section>
  );
}
