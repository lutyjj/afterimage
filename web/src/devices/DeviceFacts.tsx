import type { Device, Playlist, ScreenSummary } from "@afterimage/management-client";
import type { ReactNode } from "react";

import { Fact, Facts } from "linen";

import { ago, moment, signalQuality } from "../app/format";

/** What a device last reported and what it shows, as linen's facts;
 * `children` adds facts of the page's own after them. */
export function DeviceFacts({
  device,
  playlists,
  screens,
  timeZone,
  children,
}: {
  device: Device;
  playlists: Playlist[] | undefined;
  screens: ScreenSummary[] | undefined;
  timeZone: string | undefined;
  children?: ReactNode;
}) {
  const { showing, seenAt, wakesAt } = device;
  const { batteryVoltage, batteryPercent, signal, firmwareVersion } = device.readings;
  const screenName = (id: number) => screens?.find((screen) => screen.id === id)?.name ?? "A screen";
  const playlistName = (id: number) => playlists?.find((playlist) => playlist.id === id)?.name ?? "a playlist";
  const pinned =
    showing !== undefined &&
    ((showing.screenId !== undefined && showing.screenId === device.shows.screenId) ||
      (showing.playlistId !== undefined && showing.playlistId === device.shows.playlistId));
  const what = !showing
    ? "Nothing yet"
    : showing.asleep
      ? showing.screenId !== undefined
        ? `Asleep, showing ${screenName(showing.screenId)}`
        : "Asleep"
      : showing.screenId !== undefined
        ? screenName(showing.screenId)
        : "Afterimage's own screen";
  const from = showing?.playlistId !== undefined ? `, from ${playlistName(showing.playlistId)}` : "";
  const showingName = `${what}${from}${pinned ? ", pinned" : ""}`;

  return (
    <Facts>
      <Fact name="Showing">
        {showingName}
        {showing && timeZone && (
          <Aside>
            {" "}
            {showing.asleep && wakesAt ? (
              <>
                until <Moment at={wakesAt} timeZone={timeZone} />
              </>
            ) : (
              <>
                since <Moment at={showing.renderedAt} timeZone={timeZone} />
              </>
            )}
          </Aside>
        )}
      </Fact>
      <Fact name="Last seen">
        {seenAt ? ago(seenAt) : "Never"}
        {seenAt && timeZone && (
          <Aside>
            {" "}
            <Moment at={seenAt} timeZone={timeZone} />
          </Aside>
        )}
      </Fact>
      <Fact name="Battery">
        {batteryPercent !== undefined ? `${Math.round(batteryPercent)}%` : "Not reported"}
        {batteryVoltage !== undefined && <Aside> {batteryVoltage.toFixed(2)} V</Aside>}
      </Fact>
      <Fact name="Signal">
        {signal !== undefined ? signalQuality(signal) : "Not reported"}
        {signal !== undefined && <Aside> {signal} dBm</Aside>}
      </Fact>
      <Fact name="Firmware">
        {firmwareVersion ?? "Not reported"}
        {device.firmware.updateAvailable && <Aside> · {device.firmware.latestVersion} available</Aside>}
      </Fact>
      {children}
    </Facts>
  );
}

/** A day and time that stays on one line. */
function Moment({ at, timeZone }: { at: Date; timeZone: string }) {
  return (
    <time className="whitespace-nowrap" dateTime={at.toISOString()}>
      {moment(at, timeZone)}
    </time>
  );
}

/** A fact's detail, quieter, such as the voltage beside the battery's percent. */
export function Aside({ children }: { children: ReactNode }) {
  return <span className="text-text-muted">{children}</span>;
}
