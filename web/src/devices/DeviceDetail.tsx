import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { Page } from "../app/Page";
import { Panel } from "../panel/Panel";
import { screensQuery } from "../screens/queries";
import styles from "./DeviceDetail.module.css";
import { Aside, DeviceFacts, Fact } from "./DeviceFacts";
import { DeviceFirmware } from "./DeviceFirmware";
import { DeviceSettings } from "./DeviceSettings";
import { PlaylistEditor } from "./PlaylistEditor";
import { deviceQuery, findDisplay, imageUrl } from "./queries";
import { RemoveDevice } from "./RemoveDevice";
import { deviceStates, useShownStates } from "./states";
import { StateBadge } from "./StateBadge";

export function DeviceDetail() {
  const deviceId = Number(useParams().deviceId);
  const location = useLocation();
  const navigate = useNavigate();
  // The claim's notice shows once: the history entry forgets it, so a reload or a
  // return to this page does not claim again.
  const [claimed] = useState(() => (location.state as { claimed?: boolean; replaced?: boolean } | null) ?? null);
  useEffect(() => {
    if (location.state) void navigate(".", { replace: true, state: null });
  }, []);
  const device = useQuery(deviceQuery(deviceId));
  const models = useQuery(displayModelsQuery).data;
  const screens = useQuery(screensQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const states = useShownStates(device.data && timeZone ? deviceStates(device.data, timeZone) : []);
  const back = { to: "/", label: "Devices" };

  if (device.isPending) {
    return (
      <Page back={back} title="Loading">
        <p className="muted">Loading the display</p>
      </Page>
    );
  }
  if (device.isError) {
    return (
      <Page back={back} title="This display could not load">
        <p className="error-text">{messageOf(device.error)}</p>
        <Link to="/">Back to devices</Link>
      </Page>
    );
  }
  const data = device.data;
  const { model, palette } = findDisplay(models, data);

  return (
    <Page
      back={back}
      title={
        <span className={styles.title}>
          {data.name}
          {states.map((state) => (
            <StateBadge key={state.key} ended={state.ended} title={state.title}>
              {state.label}
            </StateBadge>
          ))}
        </span>
      }
    >
      {claimed?.claimed && (
        <p className={styles.notice} role="status">
          {claimed.replaced
            ? "This display set up again; it now uses its new code and keeps its name, settings and playlist."
            : "Claimed. The display shows its first screen at its next check-in. Give it a playlist below."}
        </p>
      )}

      <div className={styles.top}>
        <Panel
          src={imageUrl(data)}
          alt={`What ${data.name} shows`}
          width={model?.width}
          height={model?.height}
          colors={palette?.colors}
          paletteName={palette?.name}
        />
        <DeviceFacts device={data} screens={screens} timeZone={timeZone}>
          <Fact label="Hardware">
            {data.firmwareModel}
            <Aside> {data.mac}</Aside>
          </Fact>
        </DeviceFacts>
      </div>

      <PlaylistEditor device={data} screens={screens} timeZone={timeZone} />
      <DeviceSettings device={data} models={models} />
      <DeviceFirmware device={data} timeZone={timeZone} />
      <RemoveDevice device={data} />
    </Page>
  );
}
