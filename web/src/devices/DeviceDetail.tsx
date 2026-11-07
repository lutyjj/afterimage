import { useQuery } from "@tanstack/react-query";
import { buttonClass, Card, Content, EmptyState, TopBar } from "linen";
import { Link, useParams } from "react-router";

import { displayModelsQuery, messageOf, serverQuery } from "../app/client";
import { Panel } from "../panel/Panel";
import { screensQuery } from "../screens/queries";
import { Aside, DeviceFacts, Fact } from "./DeviceFacts";
import { DeviceFirmware } from "./DeviceFirmware";
import { DeviceSettings } from "./DeviceSettings";
import { PlaylistEditor } from "./PlaylistEditor";
import { deviceQuery, findDisplay, imageUrl } from "./queries";
import { RemoveDevice } from "./RemoveDevice";
import { deviceStates, useShownStates } from "./states";
import { StateBadge } from "./StateBadge";

const BACK = { to: "/devices", label: "Devices" };

export function DeviceDetail() {
  const deviceId = Number(useParams().deviceId);
  const device = useQuery(deviceQuery(deviceId));
  const models = useQuery(displayModelsQuery).data;
  const screens = useQuery(screensQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const states = useShownStates(device.data && timeZone ? deviceStates(device.data, timeZone) : []);

  if (device.isPending) {
    return <TopBar back={BACK} title="Display" />;
  }
  if (device.isError) {
    return (
      <>
        <TopBar back={BACK} title="Display" />
        <Content>
          <EmptyState title="This display could not load" detail={messageOf(device.error)}>
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
        back={BACK}
        title={data.name}
        meta={states.map((state) => (
          <StateBadge key={state.key} ended={state.ended} title={state.title}>
            {state.label}
          </StateBadge>
        ))}
      />
      <Content>
        <div className="grid gap-4">
          <Card className="grid items-start gap-6 p-gutter min-[72rem]:grid-cols-[minmax(0,800px)_minmax(15rem,1fr)]">
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
          </Card>
          <PlaylistEditor device={data} screens={screens} timeZone={timeZone} />
          <DeviceSettings device={data} models={models} />
          <DeviceFirmware device={data} timeZone={timeZone} />
          <RemoveDevice device={data} />
        </div>
      </Content>
    </>
  );
}
