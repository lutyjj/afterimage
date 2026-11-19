import { useQuery } from "@tanstack/react-query";
import { buttonClass, Card, CardBody, Content, EmptyState, Fact, TopBar } from "linen";
import { Link, useParams } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { Panel } from "../panel/Panel";
import { playlistsQuery } from "../playlists/queries";
import { screensQuery } from "../screens/queries";
import { Aside, DeviceFacts } from "./DeviceFacts";
import { DeviceFirmware } from "./DeviceFirmware";
import { DeviceSettings } from "./DeviceSettings";
import { deviceQuery, findDisplay, imageUrl } from "./queries";
import { RemoveDevice } from "./RemoveDevice";
import { ScheduleEditor } from "./ScheduleEditor";
import { SleepEditor } from "./SleepEditor";
import { deviceStates, useShownStates } from "./states";
import { StateBadge } from "./StateBadge";

const TRAIL = [{ href: "/devices", label: "Devices" }];

export function DeviceDetail() {
  const deviceId = Number(useParams().deviceId);
  const device = useQuery(deviceQuery(deviceId));
  const models = useQuery(displayModelsQuery).data;
  const screens = useQuery(screensQuery).data;
  const playlists = useQuery(playlistsQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const states = useShownStates(device.data && timeZone ? deviceStates(device.data, timeZone, playlists) : []);

  if (device.isPending) {
    return <TopBar trail={TRAIL} title="Display" />;
  }
  if (device.isError) {
    return (
      <>
        <TopBar trail={TRAIL} title="Display" />
        <Content>
          <EmptyState title="This display could not load" description={messageOf(device.error)}>
            <Link to="/devices" className={buttonClass()}>
              Back to devices
            </Link>
          </EmptyState>
        </Content>
      </>
    );
  }
  const data = device.data;
  const { model, palette } = findDisplay(models, data);

  return (
    <>
      <TopBar
        trail={TRAIL}
        title={data.name}
        meta={states.map((state) => (
          <StateBadge key={state.key} ended={state.ended} title={state.title}>
            {state.label}
          </StateBadge>
        ))}
      />
      <Content>
        <Card>
          <CardBody className="min-[72rem]:grid min-[72rem]:grid-cols-[minmax(0,800px)_minmax(15rem,1fr)] min-[72rem]:items-start">
            <Panel
              src={imageUrl(data)}
              alt={`What ${data.name} shows`}
              width={model?.width}
              height={model?.height}
              colors={palette?.colors}
              paletteName={palette?.name}
            />
            <DeviceFacts device={data} playlists={playlists} screens={screens} timeZone={timeZone}>
              <Fact name="Hardware">
                {data.firmwareModel}
                <Aside> {data.mac}</Aside>
              </Fact>
            </DeviceFacts>
          </CardBody>
        </Card>
        {/* Keyed by the display, so a draft never carries over to another. */}
        <ScheduleEditor
          key={`schedule-${data.id}`}
          device={data}
          playlists={playlists}
          screens={screens}
          timeZone={timeZone}
        />
        <SleepEditor key={`sleep-${data.id}`} device={data} screens={screens} timeZone={timeZone} />
        <DeviceSettings key={`settings-${data.id}`} device={data} models={models} />
        <DeviceFirmware key={`firmware-${data.id}`} device={data} timeZone={timeZone} />
        <RemoveDevice device={data} />
      </Content>
    </>
  );
}
