import type { Device, ScreenSummary } from "@afterimage/management-client";
import type { ReactNode } from "react";

import { ago, moment, signalQuality } from "../app/format";
import styles from "./DeviceFacts.module.css";

/** What a device last reported and what it shows, as label and value pairs;
 * `children` adds facts of the page's own after them. */
export function DeviceFacts({
  device,
  screens,
  timeZone,
  className,
  children,
}: {
  device: Device;
  screens: ScreenSummary[] | undefined;
  timeZone: string | undefined;
  className?: string | undefined;
  children?: ReactNode;
}) {
  const { showing, seenAt } = device;
  const { batteryVoltage, batteryPercent, signal, firmwareVersion } = device.readings;
  const showingName = showing
    ? showing.screenId !== undefined
      ? (screens?.find((screen) => screen.id === showing.screenId)?.name ?? "A screen")
      : "Afterimage's own screen"
    : "Nothing yet";

  return (
    <dl className={`${styles.facts} ${className ?? ""}`}>
      <Fact label="Showing">
        {showingName}
        {showing && timeZone && <Aside> since {moment(showing.renderedAt, timeZone)}</Aside>}
      </Fact>
      <Fact label="Last seen">
        {seenAt ? ago(seenAt) : "Never"}
        {seenAt && timeZone && <Aside> {moment(seenAt, timeZone)}</Aside>}
      </Fact>
      <Fact label="Battery">
        {batteryPercent !== undefined ? `${Math.round(batteryPercent)}%` : "Not reported"}
        {batteryVoltage !== undefined && <Aside> {batteryVoltage.toFixed(2)} V</Aside>}
      </Fact>
      <Fact label="Signal">
        {signal !== undefined ? signalQuality(signal) : "Not reported"}
        {signal !== undefined && <Aside> {signal} dBm</Aside>}
      </Fact>
      <Fact label="Firmware">{firmwareVersion ?? "Not reported"}</Fact>
      {children}
    </dl>
  );
}

export function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

/** A fact's detail in soft ink, such as the voltage beside the battery's percent. */
export function Aside({ children }: { children: ReactNode }) {
  return <span className={styles.aside}>{children}</span>;
}
