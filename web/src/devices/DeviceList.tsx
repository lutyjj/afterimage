import type { Device, DisplayModel, ScreenSummary } from "@afterimage/management-client";
import { useQuery } from "@tanstack/react-query";
import { Plus, X } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { Page } from "../app/Page";
import { Panel } from "../panel/Panel";
import { screensQuery } from "../screens/queries";
import { ClaimPanel } from "./ClaimPanel";
import { DayStrip } from "./DayStrip";
import { DeviceFacts, Fact } from "./DeviceFacts";
import styles from "./DeviceList.module.css";
import { devicesQuery, findDisplay, imageUrl } from "./queries";
import { nowIn } from "./schedule";
import { deviceStates, useShownStates } from "./states";
import { StateBadge } from "./StateBadge";

/** Every display with what it shows now, and the place to claim a new one. */
export function DeviceList() {
  const devices = useQuery(devicesQuery);
  const models = useQuery(displayModelsQuery).data;
  const screens = useQuery(screensQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const [claiming, setClaiming] = useState(false);
  const none = devices.data?.length === 0;

  return (
    <Page
      title="Devices"
      description={timeZone && `Playlists keep ${timeZone} time.`}
      actions={
        !none && (
          <button
            type="button"
            className="button"
            data-variant={claiming ? undefined : "primary"}
            aria-expanded={claiming}
            onClick={() => setClaiming((open) => !open)}
          >
            {claiming ? <X size={16} aria-hidden="true" /> : <Plus size={16} aria-hidden="true" />}
            {claiming ? "Close" : "Claim a display"}
          </button>
        )
      }
    >
      {devices.isPending && <p className="muted">Loading displays</p>}
      {devices.isError && <p className="error-text">{messageOf(devices.error)}</p>}
      {none && <FirstDisplay />}
      {claiming && !none && <ClaimPanel />}
      {devices.data && devices.data.length > 0 && (
        <ol className={styles.devices} aria-label="Displays">
          {devices.data.map((device) => (
            <DeviceCard key={device.id} device={device} models={models} screens={screens} timeZone={timeZone} />
          ))}
        </ol>
      )}
    </Page>
  );
}

function DeviceCard({
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
  const states = useShownStates(timeZone ? deviceStates(device, timeZone) : []);
  const screenName = (id: number) => screens?.find((screen) => screen.id === id)?.name ?? "A screen";
  const now = timeZone ? nowIn(timeZone) : undefined;

  return (
    <li className={styles.device}>
      <Link to={`/devices/${device.id}`} className={styles.panel} aria-label={`${device.name}, open`} tabIndex={-1}>
        <Panel
          src={imageUrl(device)}
          alt={`What ${device.name} shows`}
          width={model?.width}
          height={model?.height}
          colors={palette?.colors}
          paletteName={palette?.name}
        />
      </Link>
      <div className={styles.details}>
        <div className={styles.titleLine}>
          <h2 className={styles.name}>
            <Link to={`/devices/${device.id}`}>{device.name}</Link>
          </h2>
          {states.map((state) => (
            <StateBadge key={state.key} ended={state.ended} title={state.title}>
              {state.label}
            </StateBadge>
          ))}
        </div>
        <DeviceFacts device={device} screens={screens} timeZone={timeZone}>
          <Fact label="Display">{model?.label ?? device.display.model}</Fact>
        </DeviceFacts>
        {now && (
          <div className={styles.today}>
            <h3 className={styles.todayTitle}>Today</h3>
            <DayStrip
              day={now.day}
              now={now.minute}
              entries={device.playlist.map((entry) => ({ entry, name: screenName(entry.screenId) }))}
              label={`${device.name}'s playlist today`}
            />
          </div>
        )}
      </div>
    </li>
  );
}

/** How a first display joins: the server's address goes into its setup portal,
 * and the code it then shows claims it. */
function FirstDisplay() {
  return (
    <div className={styles.first}>
      <div className={styles.firstText}>
        <h2 className={styles.firstTitle}>Set up your first display</h2>
        <ol className={styles.steps}>
          <li>
            Start the display's Wi-Fi setup. In its setup portal, set the server to{" "}
            <code className={styles.origin}>{window.location.origin}</code>.
          </li>
          <li>The display joins and shows a six-character code.</li>
          <li>Enter the code here.</li>
        </ol>
      </div>
      <ClaimPanel />
    </div>
  );
}
