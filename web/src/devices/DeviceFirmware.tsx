import type { Device, FirmwareBoard } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState, type FormEvent } from "react";

import { client, messageOf, queryClient } from "../app/client";
import { moment } from "../app/format";
import styles from "./DeviceFirmware.module.css";
import { nextCheckIn, updateSteps, type Step } from "./firmwareSteps";
import { deviceQuery, devicesQuery, firmwareBoardsQuery } from "./queries";

/** The board a display is built on, TRMNL's newest release for it, and an
 * install request followed from the request to the check-in that tells whether
 * the display installed it. */
export function DeviceFirmware({ device, timeZone }: { device: Device; timeZone: string | undefined }) {
  const boards = useQuery(firmwareBoardsQuery);
  const { firmware } = device;
  const installed = device.readings.firmwareVersion;
  const update = firmware.update;
  const pending = update?.state === "requested" || update?.state === "sent";
  const [confirming, setConfirming] = useState(false);
  const savedBoard = firmware.board ?? "";
  const [board, setBoard] = useState(savedBoard);
  useEffect(() => setBoard(savedBoard), [savedBoard]);
  const choosing = board !== savedBoard;

  const stored = (updated: Device) => {
    queryClient.setQueryData(deviceQuery(updated.id).queryKey, updated);
    void queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
  };
  const install = useMutation({
    mutationFn: (version: string) => client.installFirmware({ deviceId: device.id, version }),
    onSuccess: ({ device: updated }) => {
      stored(updated);
      setConfirming(false);
    },
  });
  const cancel = useMutation({
    mutationFn: () => client.cancelFirmwareUpdate({ deviceId: device.id }),
    onSuccess: ({ device: updated }) => stored(updated),
  });

  const latest = firmware.latestVersion;
  // An install is for the saved board, so it waits while another is picked.
  const offered =
    firmware.updateAvailable && latest !== undefined && !pending && update?.state !== "not_installed" && !choosing;

  return (
    <section className={styles.section} aria-labelledby="firmware-title">
      <div className={styles.head}>
        <h2 id="firmware-title" className={styles.title}>
          Firmware
        </h2>
        <p className="hint">{lead(device)}</p>
      </div>

      <BoardChoice device={device} boards={boards.data} board={board} onChange={setBoard} onSaved={stored} />
      {boards.isError && (
        <p className="error-text" role="alert">
          {messageOf(boards.error)}
        </p>
      )}

      {update && (
        <div className={styles.request}>
          <p className={styles.requestTitle}>Update to {update.version}</p>
          <ol className={styles.steps}>
            {updateSteps(update, device).map((step) => (
              <StepItem key={step.label} step={step} timeZone={timeZone} />
            ))}
          </ol>
          {update.state === "sent" && (
            <p className="hint">
              The display has the request and installs {update.version}, then starts it: firmware 1.8 at once,
              firmware 1.6 at its next wake. It reports the release it runs when it checks in.
            </p>
          )}
          {update.state === "not_installed" && (
            <p className="hint">
              The display checked in on {installed ?? "another release"} after it had the request. Firmware 1.8
              waits a day after a failed update before it tries again, and its log on the server says why it
              failed.
            </p>
          )}
          <div className={styles.actions}>
            {update.state === "requested" && (
              <button
                className="button"
                data-variant="quiet"
                type="button"
                disabled={cancel.isPending}
                onClick={() => cancel.mutate()}
              >
                {cancel.isPending ? "Withdrawing" : "Withdraw the request"}
              </button>
            )}
            {update.state === "not_installed" && !choosing && (
              <button
                className="button"
                type="button"
                disabled={install.isPending}
                onClick={() => install.mutate(update.version)}
              >
                {install.isPending ? `Fetching ${update.version} from TRMNL` : "Try again"}
              </button>
            )}
          </div>
          {cancel.isError && (
            <p className="error-text" role="alert">
              {messageOf(cancel.error)}
            </p>
          )}
        </div>
      )}

      {offered &&
        (!confirming ? (
          <div className={styles.offer}>
            <button className="button" type="button" onClick={() => setConfirming(true)}>
              Install {latest}…
            </button>
          </div>
        ) : (
          <div className={styles.confirm}>
            <p>
              At its next check-in
              {timeZone && nextCheckIn(device) ? `, by about ${moment(nextCheckIn(device)!, timeZone)},` : ""} the
              display downloads {latest} from this server, shows that it is updating and installs it. It keeps its
              Wi-Fi, its settings and its playlist.
            </p>
            <p className="hint">
              Afterimage first fetches the release from TRMNL and checks it, which takes a few seconds.
            </p>
            <div className={styles.actions}>
              <button
                className="button"
                data-variant="primary"
                type="button"
                disabled={install.isPending}
                onClick={() => install.mutate(latest)}
              >
                {install.isPending ? `Fetching ${latest} from TRMNL` : `Install ${latest}`}
              </button>
              <button
                className="button"
                data-variant="quiet"
                type="button"
                disabled={install.isPending}
                onClick={() => setConfirming(false)}
              >
                Not now
              </button>
            </div>
          </div>
        ))}
      {install.isError && (
        <p className="error-text" role="alert">
          {messageOf(install.error)}
        </p>
      )}
    </section>
  );
}

/** What the display runs against what TRMNL offers its board. */
function lead(device: Device): string {
  const { firmware } = device;
  const installed = device.readings.firmwareVersion;
  const runs = installed ? `The display runs ${installed}.` : "The display has not reported its firmware yet.";
  if (!firmware.board) return `${runs} Choose the board it is built on to install TRMNL's releases.`;
  if (!firmware.latestVersion) return runs;
  if (installed === firmware.latestVersion) {
    return `The display runs ${installed}, TRMNL's newest release for this board.`;
  }
  return `${runs} TRMNL's newest release for this board is ${firmware.latestVersion}.`;
}

function StepItem({ step, timeZone }: { step: Step; timeZone: string | undefined }) {
  const when = step.at && timeZone ? moment(step.at, timeZone) : undefined;
  return (
    <li className={styles.step} data-state={step.state} aria-current={step.state === "current" ? "step" : undefined}>
      <span className={styles.stepLabel}>{step.label}</span>
      <span className={styles.stepWhen}>
        {step.state === "current" && step.note ? (when ? `${step.note}, by ${when}` : step.note) : (when ?? step.note)}
      </span>
    </li>
  );
}

/** Where the board came from, and while one is being chosen, what a wrong one
 * does. */
function boardHint(device: Device, choosing: boolean): string {
  const model = device.firmwareModel;
  const risk = "Firmware for another board can leave the display unable to start until it is flashed over USB.";
  if (choosing) {
    const unlinked = model
      ? `Its firmware reports the model ${model}, which TRMNL does not link to a board.`
      : "Its firmware has not reported its model yet.";
    return `${unlinked} Choose the one the display is built on. ${risk}`;
  }
  return device.firmware.boardChosen
    ? `Chosen here. Its firmware reports the model ${model}.`
    : `Matched by the model its firmware reports, ${model}.`;
}

/** The board select, saved on its own: choosing another board withdraws a
 * request the display has not received. */
function BoardChoice({
  device,
  boards,
  board,
  onChange,
  onSaved,
}: {
  device: Device;
  boards: FirmwareBoard[] | undefined;
  board: string;
  onChange: (board: string) => void;
  onSaved: (device: Device) => void;
}) {
  const current = device.firmware.board ?? "";
  const save = useMutation({
    mutationFn: () => client.updateDevice({ deviceId: device.id, firmwareBoard: board }),
    onSuccess: ({ device: updated }) => onSaved(updated),
  });
  const sorted = [...(boards ?? [])].sort((a, b) => a.label.localeCompare(b.label));
  const changed = board !== "" && board !== current;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (changed) save.mutate();
  };

  return (
    <form className={styles.board} onSubmit={submit}>
      <label className="field">
        <span>Board</span>
        <select
          className="select"
          value={board}
          disabled={!boards}
          onChange={(event) => onChange(event.target.value)}
        >
          {board === "" && (
            <option value="" disabled>
              {boards ? "Choose a board" : "Reading TRMNL's list"}
            </option>
          )}
          {board !== "" && !sorted.some((candidate) => candidate.name === board) && (
            <option value={board}>{board}</option>
          )}
          {sorted.map((candidate) => (
            <option key={candidate.name} value={candidate.name}>
              {candidate.label} · {candidate.chipFamily}
            </option>
          ))}
        </select>
        <span className="hint">{boardHint(device, board === "" || changed)}</span>
      </label>
      {changed && (
        <div className={styles.actions}>
          <button className="button" data-variant="primary" type="submit" disabled={save.isPending}>
            {save.isPending ? "Saving" : "Use this board"}
          </button>
        </div>
      )}
      {save.isError && (
        <p className="error-text" role="alert">
          {messageOf(save.error)}
        </p>
      )}
    </form>
  );
}
