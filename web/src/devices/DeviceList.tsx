import type { Device, DisplayModel, ScreenSummary } from "@afterimage/management-client";
import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { CropMarks } from "../proof/CropMarks";
import { Proof } from "../proof/Proof";
import { Stamp } from "../proof/Stamp";
import { screensQuery } from "../screens/queries";
import { ClaimPanel } from "./ClaimPanel";
import { DayStrip } from "./DayStrip";
import { DeviceFacts, Fact } from "./DeviceFacts";
import styles from "./DeviceList.module.css";
import { devicesQuery, findDisplay, imageUrl } from "./queries";
import { nowIn } from "./schedule";
import { deviceStates, useStamps } from "./states";

/** Every display as its current proof, and the place to claim a new one. */
export function DeviceList() {
  const devices = useQuery(devicesQuery);
  const models = useQuery(displayModelsQuery).data;
  const screens = useQuery(screensQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;

  return (
    <div className={styles.page}>
      <header className={styles.head}>
        <h1 className={styles.heading}>Devices</h1>
        {timeZone && <p className="annotation">Times in {timeZone}</p>}
      </header>
      <div className={styles.layout}>
        <aside className={styles.claim}>
          <ClaimPanel />
        </aside>
        <section className={styles.list} aria-label="Displays">
          {devices.isPending && <p className="annotation">Loading displays</p>}
          {devices.isError && <p className="error-text">{messageOf(devices.error)}</p>}
          {devices.data?.length === 0 && <FirstDisplay />}
          <ol className={styles.rows}>
            {devices.data?.map((device) => (
              <DeviceRow
                key={device.id}
                device={device}
                models={models}
                screens={screens}
                timeZone={timeZone}
              />
            ))}
          </ol>
        </section>
      </div>
    </div>
  );
}

function DeviceRow({
  device,
  models,
  screens,
  timeZone,
}: {
  device: Device;
  models: DisplayModel[] | undefined;
  screens: ScreenSummary[] | undefined;
  timeZone: string | undefined;
}) {
  const { model, palette } = findDisplay(models, device);
  const stamps = useStamps(timeZone ? deviceStates(device, timeZone) : []);
  const screenName = (id: number) => screens?.find((screen) => screen.id === id)?.name ?? "A screen";
  const now = timeZone ? nowIn(timeZone) : undefined;

  return (
    <li className={styles.row}>
      <Link to={`/devices/${device.id}`} className={styles.proofLink} aria-label={`${device.name}, open`}>
        <Proof
          src={imageUrl(device)}
          alt={`What ${device.name} shows`}
          width={model?.width}
          height={model?.height}
          colors={palette?.colors}
          paletteName={palette?.name}
        />
      </Link>
      <div className={styles.meta}>
        <div className={styles.titleLine}>
          <h2 className={styles.name}>
            <Link to={`/devices/${device.id}`}>{device.name}</Link>
          </h2>
          {stamps.length > 0 && (
            <div className={styles.stamps}>
              {stamps.map((stamp) => (
                <Stamp key={stamp.key} cancelled={stamp.cancelled} title={stamp.title}>
                  {stamp.label}
                </Stamp>
              ))}
            </div>
          )}
        </div>
        <DeviceFacts device={device} screens={screens} timeZone={timeZone} className={styles.facts}>
          <Fact label="Display">{model?.label ?? device.display.model}</Fact>
        </DeviceFacts>
      </div>
      {now && (
        <div className={styles.today}>
          <span className="annotation">Today</span>
          <DayStrip
            day={now.day}
            now={now.minute}
            entries={device.playlist.map((entry) => ({ entry, name: screenName(entry.screenId) }))}
            label={`${device.name}'s playlist today`}
          />
        </div>
      )}
    </li>
  );
}

/** How a first display joins: the server's address goes into its setup portal. */
function FirstDisplay() {
  const origin = window.location.origin;
  return (
    <div className={styles.first}>
      <CropMarks />
      <h2 className={styles.firstTitle}>No displays yet</h2>
      <ol className={styles.steps}>
        <li>
          <span>
            Start the display's Wi-Fi setup. In its setup portal, set the server to{" "}
            <code className={styles.origin}>{origin}</code>.
          </span>
        </li>
        <li>
          <span>The display joins and shows a six-character code.</span>
        </li>
        <li>
          <span>Enter the code under “Claim a display”.</span>
        </li>
      </ol>
    </div>
  );
}
