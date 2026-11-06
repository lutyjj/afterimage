import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { Proof } from "../proof/Proof";
import { Stamp } from "../proof/Stamp";
import { screensQuery } from "../screens/queries";
import styles from "./DeviceDetail.module.css";
import { Aside, DeviceFacts, Fact } from "./DeviceFacts";
import { DeviceSettings } from "./DeviceSettings";
import { PlaylistEditor } from "./PlaylistEditor";
import { deviceQuery, findDisplay, imageUrl } from "./queries";
import { RemoveDevice } from "./RemoveDevice";
import { deviceStates, useStamps } from "./states";

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

  return (
    <div className={styles.page}>
      <nav className={styles.trail} aria-label="Breadcrumb">
        <Link to="/">
            <ArrowLeft size={14} aria-hidden="true" />
            Devices
          </Link>
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
          width={model?.width}
          height={model?.height}
          colors={palette?.colors}
          paletteName={palette?.name}
        />
        <DeviceFacts device={data} screens={screens} timeZone={timeZone} className={styles.facts}>
          <Fact label="Hardware">
            {data.firmwareModel}
            <Aside> {data.mac}</Aside>
          </Fact>
        </DeviceFacts>
      </div>

      <PlaylistEditor device={data} screens={screens} timeZone={timeZone} />
      <DeviceSettings device={data} models={models} />
      <RemoveDevice device={data} />
    </div>
  );
}
