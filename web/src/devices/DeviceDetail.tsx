import { useQuery } from "@tanstack/react-query";
import { Link, useLocation, useParams } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { ago, moment, signalQuality } from "../app/format";
import { Proof } from "../proof/Proof";
import { Stamp } from "../proof/Stamp";
import { screensQuery } from "../screens/queries";
import styles from "./DeviceDetail.module.css";
import { DeviceSettings } from "./DeviceSettings";
import { PlaylistEditor } from "./PlaylistEditor";
import { deviceQuery, findDisplay, imageUrl } from "./queries";
import { RemoveDevice } from "./RemoveDevice";
import { deviceStates, useStamps } from "./states";

export function DeviceDetail() {
  const deviceId = Number(useParams().deviceId);
  const location = useLocation();
  const claimed = (location.state as { claimed?: boolean; replaced?: boolean } | null) ?? null;
  const device = useQuery(deviceQuery(deviceId));
  const models = useQuery(displayModelsQuery).data;
  const screens = useQuery(screensQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const stamps = useStamps(device.data && timeZone ? deviceStates(device.data, timeZone) : []);

  if (device.isPending) {
    return <p className={`${styles.page} annotation`}>Loading</p>;
  }
  if (device.isError) {
    return (
      <div className={styles.page}>
        <p className="error-text">{messageOf(device.error)}</p>
        <Link to="/">Back to devices</Link>
      </div>
    );
  }
  const data = device.data;
  const { model, palette } = findDisplay(models, data);
  const { batteryVoltage, batteryPercent, signal, firmwareVersion } = data.readings;
  const showing = data.showing;
  const showingName = showing
    ? showing.screenId !== undefined
      ? (screens?.find((screen) => screen.id === showing.screenId)?.name ?? "A screen")
      : "Afterimage's own screen"
    : "Nothing yet";

  return (
    <div className={styles.page}>
      <nav className={styles.trail} aria-label="Breadcrumb">
        <Link to="/">Devices</Link>
      </nav>
      <header className={styles.head}>
        <h1 className={styles.name}>{data.name}</h1>
        {stamps.map((stamp) => (
          <Stamp key={stamp.key} cancelled={stamp.cancelled} title={stamp.title}>
            {stamp.label}
          </Stamp>
        ))}
      </header>
      {claimed?.claimed && (
        <p className={styles.notice} role="status">
          {claimed.replaced
            ? "This display set up again; it now uses its new code and keeps its name, settings and playlist."
            : "Claimed. The display shows its first screen at its next check-in. Give it a playlist below."}
        </p>
      )}

      <div className={styles.top}>
        <Proof
          src={imageUrl(data)}
          alt={`What ${data.name} shows`}
          width={model?.width ?? 800}
          height={model?.height ?? 480}
          colors={palette?.colors}
          paletteName={palette?.name}
        />
        <dl className={styles.facts}>
          <div>
            <dt>Showing</dt>
            <dd>
              {showingName}
              {showing && timeZone && <span className={styles.aside}> since {moment(showing.renderedAt, timeZone)}</span>}
            </dd>
          </div>
          <div>
            <dt>Last seen</dt>
            <dd>
              {data.seenAt ? ago(data.seenAt) : "Never"}
              {data.seenAt && timeZone && <span className={styles.aside}> {moment(data.seenAt, timeZone)}</span>}
            </dd>
          </div>
          <div>
            <dt>Battery</dt>
            <dd>
              {batteryPercent !== undefined ? `${Math.round(batteryPercent)}%` : "Not reported"}
              {batteryVoltage !== undefined && <span className={styles.aside}> {batteryVoltage.toFixed(2)} V</span>}
            </dd>
          </div>
          <div>
            <dt>Signal</dt>
            <dd>
              {signal !== undefined ? signalQuality(signal) : "Not reported"}
              {signal !== undefined && <span className={styles.aside}> {signal} dBm</span>}
            </dd>
          </div>
          <div>
            <dt>Firmware</dt>
            <dd>{firmwareVersion ?? "Not reported"}</dd>
          </div>
          <div>
            <dt>Hardware</dt>
            <dd>
              {data.firmwareModel}
              <span className={styles.aside}> {data.mac}</span>
            </dd>
          </div>
        </dl>
      </div>

      <PlaylistEditor device={data} screens={screens} timeZone={timeZone} />
      <DeviceSettings device={data} models={models} />
      <RemoveDevice device={data} />
    </div>
  );
}
