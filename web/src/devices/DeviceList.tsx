import type { Device, DisplayModel, ScreenSummary } from "@afterimage/management-client";
import { useQuery } from "@tanstack/react-query";
import { Button, Card, CardBody, CardHead, Content, TopBar } from "linen";
import { Plus } from "lucide-react";
import { type ReactNode, useState } from "react";
import { Link } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { Panel } from "../panel/Panel";
import { screensQuery } from "../screens/queries";
import { ClaimDialog } from "./ClaimDialog";
import { DayStrip } from "./DayStrip";
import { DeviceFacts, Fact } from "./DeviceFacts";
import { devicesQuery, findDisplay, imageUrl } from "./queries";
import { nowIn } from "./schedule";
import { deviceStates, useShownStates } from "./states";
import { StateBadge } from "./StateBadge";

/** Every display with what it shows now, and the way to claim a new one. */
export function DeviceList() {
  const devices = useQuery(devicesQuery);
  const models = useQuery(displayModelsQuery).data;
  const screens = useQuery(screensQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const [claiming, setClaiming] = useState(false);
  const none = devices.data?.length === 0;

  return (
    <>
      <TopBar title="Devices">
        {!none && (
          <Button variant="primary" onClick={() => setClaiming(true)}>
            <Plus aria-hidden="true" className="size-4" />
            Claim a display
          </Button>
        )}
      </TopBar>
      <Content>
        {devices.isPending && <p className="text-sm text-text-muted">Reading displays</p>}
        {devices.isError && (
          <p role="alert" className="text-sm text-danger">
            {messageOf(devices.error)}
          </p>
        )}
        {none && <FirstDisplay onClaim={() => setClaiming(true)} />}
        {devices.data && devices.data.length > 0 && (
          <ol className="grid gap-4" aria-label="Displays">
            {devices.data.map((device) => (
              <DeviceCard key={device.id} device={device} models={models} screens={screens} timeZone={timeZone} />
            ))}
          </ol>
        )}
      </Content>
      <ClaimDialog open={claiming} onOpenChange={setClaiming} />
    </>
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
    <li>
      <Card className="grid items-start gap-6 p-gutter min-[72rem]:grid-cols-[minmax(0,800px)_minmax(15rem,1fr)]">
        <Link to={`/devices/${device.id}`} className="block min-w-0" aria-label={`${device.name}, open`} tabIndex={-1}>
          <Panel
            src={imageUrl(device)}
            alt={`What ${device.name} shows`}
            width={model?.width}
            height={model?.height}
            colors={palette?.colors}
            paletteName={palette?.name}
          />
        </Link>
        <div className="grid min-w-0 content-start gap-4">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="mr-1 font-display text-lg font-semibold tracking-[-0.01em]">
              <Link to={`/devices/${device.id}`} className="hover:text-accent">
                {device.name}
              </Link>
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
          {now && timeZone && (
            <div className="grid gap-2 border-t border-border pt-4">
              <h3 className="text-sm font-medium text-text-secondary">
                Today <span className="font-normal text-text-muted">in {timeZone}</span>
              </h3>
              <DayStrip
                day={now.day}
                now={now.minute}
                entries={device.playlist.map((entry) => ({ entry, name: screenName(entry.screenId) }))}
                label={`${device.name}'s playlist today`}
              />
            </div>
          )}
        </div>
      </Card>
    </li>
  );
}

/** How a first display joins: the server's address goes into its setup portal,
 * and the code it then shows claims it. */
function FirstDisplay({ onClaim }: { onClaim: () => void }) {
  return (
    <Card className="max-w-2xl">
      <CardHead title="Set up your first display" />
      <CardBody>
        <ol className="grid gap-3 text-sm text-text-secondary [counter-reset:step]">
          <Step>
            Start the display's Wi-Fi setup. In its setup portal, set the server to{" "}
            <code className="rounded-input bg-sunken px-1.5 py-0.5 font-mono text-xs text-text [overflow-wrap:anywhere]">
              {window.location.origin}
            </code>
            .
          </Step>
          <Step>The display joins and shows a six-character code.</Step>
          <Step>
            <span className="grid justify-items-start gap-3">
              Claim it with that code.
              <Button variant="primary" onClick={onClaim}>
                <Plus aria-hidden="true" className="size-4" />
                Claim a display
              </Button>
            </span>
          </Step>
        </ol>
      </CardBody>
    </Card>
  );
}

function Step({ children }: { children: ReactNode }) {
  return (
    <li className="grid grid-cols-[1.75rem_minmax(0,1fr)] [counter-increment:step] before:grid before:size-5.5 before:place-items-center before:rounded-full before:bg-accent-weak before:text-xs before:font-semibold before:text-accent before:content-[counter(step)]">
      <span className="pt-px">{children}</span>
    </li>
  );
}
