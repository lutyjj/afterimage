import {
  Forbidden,
  type MqttSettings,
  type UpdateMqttSettingsInput,
  type UpdateMqttSettingsOutput,
} from "@afterimage/management-client";
import { useMutation, useQuery, type UseMutationResult } from "@tanstack/react-query";
import {
  Actions,
  Button,
  Card,
  CardBody,
  CardHead,
  Checkbox,
  Chip,
  ConfirmDialog,
  Nothing,
  SettingNote,
  SettingRow,
  TextField,
  useToast,
  WarnDot,
} from "linen";
import { type FormEvent, useState } from "react";

import { client, messageOf, queryClient } from "../app/client";
import { mqttQuery } from "./queries";

type Save = UseMutationResult<UpdateMqttSettingsOutput, Error, UpdateMqttSettingsInput>;

/** The MQTT broker that carries every display to Home Assistant, which only the
 * server's administrator, its first account, reads and changes. */
export function HomeAssistant() {
  const mqtt = useQuery(mqttQuery);
  const toast = useToast();
  // Kept here, since the form starts afresh from what the server saved.
  const save = useMutation({
    mutationFn: (request: UpdateMqttSettingsInput) => client.updateMqttSettings(request),
    onSuccess: ({ settings }) => {
      queryClient.setQueryData(mqttQuery.queryKey, settings);
      toast.show({ text: settings.url ? "Broker saved" : "Stopped publishing to Home Assistant" });
    },
  });

  return (
    <Card>
      <CardHead
        title="Home Assistant"
        description="Afterimage publishes every claimed display to Home Assistant through MQTT discovery: battery, signal, last check-in, the screen it shows, and a firmware update Home Assistant can install."
      />
      {mqtt.isPending && <Nothing>Reading the broker</Nothing>}
      {mqtt.isError && (
        <CardBody>
          <p role="alert" className="text-sm text-danger">
            {mqtt.error instanceof Forbidden
              ? "Only the server's administrator, its first account, sees these settings."
              : messageOf(mqtt.error)}
          </p>
        </CardBody>
      )}
      {mqtt.data && <Connection settings={mqtt.data} />}
      {mqtt.data?.setByDeployment && <SetByDeployment settings={mqtt.data} />}
      {mqtt.data && !mqtt.data.setByDeployment && (
        <>
          <BrokerForm key={JSON.stringify({ ...mqtt.data, connection: undefined })} settings={mqtt.data} save={save} />
          {mqtt.data.url !== undefined && <StopPublishing save={save} />}
        </>
      )}
    </Card>
  );
}

const CONNECTION = {
  connected: { label: "Connected", detail: undefined },
  connecting: { label: "Connecting", detail: "Trying again every few seconds until the broker answers." },
  refused: { label: "Refused", detail: "The broker answered and refused: check the username and password." },
} as const;

/** Where the displays go, and whether they get there. */
function Connection({ settings }: { settings: MqttSettings }) {
  if (settings.connection === "off") {
    return (
      <SettingRow title="Broker" description="Save a broker below to publish the displays.">
        <Chip size="row" tone="muted">
          Not publishing
        </Chip>
      </SettingRow>
    );
  }
  const state = CONNECTION[settings.connection];
  return (
    <div aria-live="polite">
      <SettingRow
        title="Broker"
        description={
          <span className="grid">
            <code className="font-mono text-xs break-all text-text">{settings.url}</code>
            {state.detail}
          </span>
        }
      >
        {settings.connection === "connected" ? (
          <Chip size="row">
            <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-positive" />
            {state.label}
          </Chip>
        ) : (
          <Chip size="row" tone="warn">
            <WarnDot />
            {state.label}
          </Chip>
        )}
      </SettingRow>
    </div>
  );
}

/** The deployment's broker, shown as it is: it changes where the deployment sets it. */
function SetByDeployment({ settings }: { settings: MqttSettings }) {
  const none = <span className="text-text-muted">None</span>;
  return (
    <>
      <SettingRow title="Username">
        <span className="text-sm">{settings.username ?? none}</span>
      </SettingRow>
      <SettingRow title="Password">
        <span className="text-sm">{settings.passwordSet ? "Set" : none}</span>
      </SettingRow>
      <SettingRow title="Topic" description="Names the server at the broker and starts every display's topics.">
        <code className="font-mono text-xs">{settings.topic}</code>
      </SettingRow>
      <SettingRow title="Discovery prefix" description="Where Home Assistant listens for new devices.">
        <code className="font-mono text-xs">{settings.discoveryPrefix}</code>
      </SettingRow>
      <SettingNote>
        The deployment sets this broker with <code className="font-mono">AFTERIMAGE_MQTT_URL</code> and the settings
        beside it. Change it there and restart Afterimage.
      </SettingNote>
    </>
  );
}

function BrokerForm({ settings, save }: { settings: MqttSettings; save: Save }) {
  const [url, setUrl] = useState(settings.url ?? "");
  const [username, setUsername] = useState(settings.username ?? "");
  const [password, setPassword] = useState("");
  const [removePassword, setRemovePassword] = useState(false);
  const [topic, setTopic] = useState(settings.topic);
  const [prefix, setPrefix] = useState(settings.discoveryPrefix);

  const publishing = settings.url !== undefined;
  const submit = (event: FormEvent) => {
    event.preventDefault();
    save.mutate({
      url: url.trim(),
      username: username.trim() || undefined,
      // Empty keeps the saved password, and removing sends it empty.
      password: removePassword ? "" : password || undefined,
      topic: topic.trim(),
      discoveryPrefix: prefix.trim(),
    });
  };
  const movesDevices = publishing && (topic.trim() !== settings.topic || prefix.trim() !== settings.discoveryPrefix);
  // The server sends a saved password only to the broker and username it was
  // given for, so naming another asks for it again.
  const needsPassword =
    settings.passwordSet &&
    !removePassword &&
    !password &&
    (url.trim() !== settings.url || (username.trim() || undefined) !== settings.username);

  return (
    <CardBody className="border-t border-border">
      <form onSubmit={submit} className="grid max-w-2xl gap-x-4 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <TextField
            label="Broker"
            hint={
              <>
                <code className="font-mono">mqtt://</code>, or <code className="font-mono">mqtts://</code> for TLS, then
                the host and its port.
              </>
            }
            value={url}
            required
            inputMode="url"
            autoComplete="off"
            spellCheck={false}
            placeholder="mqtt://broker.example:1883"
            onChange={(event) => setUrl(event.target.value)}
          />
        </div>
        <TextField
          label="Username"
          value={username}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setUsername(event.target.value)}
        />
        <TextField
          label="Password"
          hint={needsPassword ? "Another broker or username needs the password again." : undefined}
          type="password"
          value={password}
          disabled={removePassword}
          autoComplete="new-password"
          placeholder={settings.passwordSet ? "Saved; type to replace it" : ""}
          aria-invalid={needsPassword || undefined}
          onChange={(event) => setPassword(event.target.value)}
        />
        {settings.passwordSet && (
          <div className="mb-4 sm:col-span-2">
            <Checkbox checked={removePassword} onCheckedChange={setRemovePassword}>
              Remove the saved password
            </Checkbox>
          </div>
        )}
        <TextField
          label="Topic"
          hint="Names the server at the broker and starts every display's topics."
          value={topic}
          required
          maxLength={64}
          pattern="[A-Za-z0-9_\-]+"
          spellCheck={false}
          onChange={(event) => setTopic(event.target.value)}
        />
        <TextField
          label="Discovery prefix"
          hint="Where Home Assistant listens for new devices."
          value={prefix}
          required
          maxLength={128}
          spellCheck={false}
          onChange={(event) => setPrefix(event.target.value)}
        />
        {movesDevices && (
          <p className="mb-4 rounded-button bg-sunken px-3 py-2 text-sm text-text-secondary sm:col-span-2">
            Saving takes every display out of Home Assistant under the old topic and adds it again under the new one, as
            a new device.
          </p>
        )}
        <Actions className="sm:col-span-2">
          <Button type="submit" variant="primary" disabled={save.isPending}>
            {save.isPending ? "Saving" : publishing ? "Save" : "Save and connect"}
          </Button>
          {save.isError && (
            <span role="alert" className="text-sm text-danger">
              {messageOf(save.error)}
            </span>
          )}
        </Actions>
      </form>
    </CardBody>
  );
}

/** Taking every display out of Home Assistant, said before it happens. */
function StopPublishing({ save }: { save: Save }) {
  const [confirming, setConfirming] = useState(false);
  return (
    <SettingRow
      title="Stop publishing"
      description="Every display leaves Home Assistant, with its entities and their history there, until a broker is saved again."
    >
      <Button variant="destructive" disabled={save.isPending} onClick={() => setConfirming(true)}>
        Stop publishing
      </Button>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Stop publishing to Home Assistant?"
        detail="Every display leaves Home Assistant, with its entities and their history there, until a broker is saved again."
        confirmLabel="Stop publishing"
        destructive
        onConfirm={() => save.mutate({})}
      />
    </SettingRow>
  );
}
