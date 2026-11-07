import type { Device, FirmwareBoard } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Actions, Button, Card, CardBody, CardHead, classes, ConfirmDialog, FieldRow, Select, useToast } from "linen";
import { useEffect, useState, type FormEvent } from "react";

import { client, messageOf, queryClient } from "../app/client";
import { moment } from "../app/format";
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
  const toast = useToast();
  const install = useMutation({
    mutationFn: (version: string) => client.installFirmware({ deviceId: device.id, version }),
    onSuccess: ({ device: updated }, version) => {
      stored(updated);
      toast.show({ text: `${device.name} installs ${version} at its next check-in.` });
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

  const due = timeZone && nextCheckIn(device) ? moment(nextCheckIn(device)!, timeZone) : undefined;

  return (
    <Card>
      <CardHead title="Firmware" description={lead(device)}>
        {offered && (
          <Button variant="primary" disabled={install.isPending} onClick={() => setConfirming(true)}>
            {install.isPending ? `Fetching ${latest} from TRMNL` : `Install ${latest}`}
          </Button>
        )}
      </CardHead>
      <CardBody className="grid gap-5">
        <BoardChoice
          device={device}
          boards={boards.data}
          board={board}
          locked={install.isPending}
          onChange={setBoard}
          onSaved={stored}
        />
        {boards.isError && (
          <p role="alert" className="text-sm text-danger">
            {messageOf(boards.error)}
          </p>
        )}

        {update && (
          <div className="grid max-w-2xl gap-3">
            <p className="text-sm font-semibold">Update to {update.version}</p>
            <ol className="grid grid-cols-3 gap-2">
              {updateSteps(update, device).map((step) => (
                <StepItem key={step.label} step={step} timeZone={timeZone} />
              ))}
            </ol>
            {update.state === "sent" && (
              <p className="text-sm text-text-secondary">
                The display has the request and installs {update.version}. Its next check-in shows whether it runs the
                new release.
              </p>
            )}
            {update.state === "not_installed" && (
              <p className="text-sm text-text-secondary">
                The display checked in on {installed ?? "another release"} after it had the request. Firmware 1.8 waits
                a day after a failed update before it tries again, and its log on the server says why it failed.
              </p>
            )}
            {(update.state === "requested" || (update.state === "not_installed" && !choosing)) && (
              <Actions>
                {update.state === "requested" && (
                  <Button disabled={cancel.isPending} onClick={() => cancel.mutate()}>
                    {cancel.isPending ? "Withdrawing" : "Withdraw the request"}
                  </Button>
                )}
                {update.state === "not_installed" && !choosing && (
                  <Button disabled={install.isPending} onClick={() => install.mutate(update.version)}>
                    {install.isPending ? `Fetching ${update.version} from TRMNL` : "Try again"}
                  </Button>
                )}
              </Actions>
            )}
            {cancel.isError && (
              <p role="alert" className="text-sm text-danger">
                {messageOf(cancel.error)}
              </p>
            )}
          </div>
        )}
        {install.isError && (
          <p role="alert" className="text-sm text-danger">
            {messageOf(install.error)}
          </p>
        )}
      </CardBody>
      {latest !== undefined && (
        <ConfirmDialog
          open={confirming}
          onOpenChange={setConfirming}
          title={`Install ${latest} on ${device.name}?`}
          detail={
            <>
              At its next check-in{due ? `, by about ${due},` : ""} the display downloads {latest} from this server,
              shows that it is updating and installs it. It keeps its Wi-Fi, its settings and its playlist. Afterimage
              first fetches the release from TRMNL and checks it, which takes a few seconds.
            </>
          }
          confirmLabel={`Install ${latest}`}
          onConfirm={() => install.mutate(latest)}
        />
      )}
    </Card>
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
    <li
      aria-current={step.state === "current" ? "step" : undefined}
      className={classes(
        "grid content-start gap-0.5 border-t-3 pt-2",
        step.state === "done" && "border-accent",
        step.state === "current" &&
          "border-dashed border-[color-mix(in_oklch,var(--color-accent)_45%,var(--color-border))]",
        step.state === "waiting" && "border-border",
        step.state === "failed" && "border-danger",
      )}
    >
      <span
        className={classes(
          "text-sm font-medium",
          step.state === "waiting" && "text-text-muted",
          step.state === "failed" && "font-semibold text-danger",
        )}
      >
        {step.label}
      </span>
      <span
        className={classes(
          "text-xs",
          step.state === "current" || step.state === "waiting" ? "text-text-muted" : "text-text-secondary",
        )}
      >
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
  locked,
  onChange,
  onSaved,
}: {
  device: Device;
  boards: FirmwareBoard[] | undefined;
  board: string;
  /** While an install fetches the current board's release. */
  locked: boolean;
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

  const choices = [
    ...(board !== "" && !sorted.some((candidate) => candidate.name === board) ? [{ value: board, label: board }] : []),
    ...sorted.map((candidate) => ({ value: candidate.name, label: `${candidate.label} · ${candidate.chipFamily}` })),
  ];

  return (
    <form onSubmit={submit} className="grid max-w-2xl gap-2">
      <FieldRow>
        <Select
          label="Board"
          width={104}
          placeholder={board === "" ? (boards ? "Choose a board" : "Reading TRMNL's list") : undefined}
          choices={choices}
          value={board}
          disabled={!boards || locked}
          onValueChange={onChange}
        />
        {changed && (
          <Button type="submit" variant="primary" disabled={save.isPending}>
            {save.isPending ? "Saving" : "Use this board"}
          </Button>
        )}
      </FieldRow>
      <p className="text-xs text-text-secondary">{boardHint(device, board === "" || changed)}</p>
      {save.isError && (
        <p role="alert" className="text-sm text-danger">
          {messageOf(save.error)}
        </p>
      )}
    </form>
  );
}
