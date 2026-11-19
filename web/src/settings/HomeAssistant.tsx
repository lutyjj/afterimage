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
  ConfirmDialog,
  Fact,
  Facts,
  FieldRow,
  Nothing,
  Notice,
  SettingNote,
  SettingRow,
  Status,
  TextField,
  useToast,
  useUnsavedChanges,
} from "linen";
import { type FormEvent, useState } from "react";

import { FIELD_WIDTH, PAIR_WIDTH } from "../app/fieldWidth";
import { client, messageOf, queryClient } from "../app/client";
import { ACT_SECONDS, within } from "../app/deadline";
import { mqttQuery } from "./queries";

type Save = UseMutationResult<UpdateMqttSettingsOutput, Error, UpdateMqttSettingsInput>;

/** The MQTT broker that carries every display to Home Assistant, which only the
 * server's administrator, its first account, reads and changes. */
export function HomeAssistant() {
  const mqtt = useQuery(mqttQuery);
  const toast = useToast();
  // Kept here, since the form starts afresh from what the server saved.
  const save = useMutation({
    mutationFn: (request: UpdateMqttSettingsInput) =>
      within(ACT_SECONDS, (options) => client.updateMqttSettings(request, options)),
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
      {mqtt.isError &&
        (mqtt.error instanceof Forbidden ? (
          <Nothing>Only the server's administrator, its first account, sees the broker.</Nothing>
        ) : (
          <CardBody>
            <Notice tone="danger" title={messageOf(mqtt.error)} />
          </CardBody>
        ))}
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
        <Status>Not publishing</Status>
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
        <Status tone={settings.connection === "connected" ? "positive" : "warn"}>{state.label}</Status>
      </SettingRow>
    </div>
  );
}

/** The deployment's broker, shown as it is: it changes where the deployment sets it. */
function SetByDeployment({ settings }: { settings: MqttSettings }) {
  const none = <span className="text-text-muted">None</span>;
  return (
    <>
      <CardBody>
        <Facts>
          <Fact name="Username">{settings.username ?? none}</Fact>
          <Fact name="Password">{settings.passwordSet ? "Set" : none}</Fact>
          <Fact name="Topic" mono>
            {settings.topic}
          </Fact>
          <Fact name="Discovery prefix" mono>
            {settings.discoveryPrefix}
          </Fact>
        </Facts>
      </CardBody>
      <SettingNote>
        The topic names the server at the broker and starts every display's topics, and Home Assistant listens for new
        devices under the discovery prefix. The deployment sets this broker with{" "}
        <code className="font-mono">AFTERIMAGE_MQTT_URL</code> and the settings beside it. Change it there and restart
        Afterimage.
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
    save.mutate(
      {
        url: url.trim(),
        username: username.trim() || undefined,
        // Empty keeps the saved password, and removing sends it empty.
        password: removePassword ? "" : password || undefined,
        topic: topic.trim(),
        discoveryPrefix: prefix.trim(),
      },
      {
        // The server never answers with the password, so a save that changes
        // only the password leaves the settings, and this form, as they were.
        onSuccess: () => {
          setPassword("");
          setRemovePassword(false);
        },
      },
    );
  };
  // A save of anything else remounts the form from what the server saved.
  const unsaved =
    url.trim() !== (settings.url ?? "") ||
    username.trim() !== (settings.username ?? "") ||
    password !== "" ||
    removePassword ||
    topic.trim() !== settings.topic ||
    prefix.trim() !== settings.discoveryPrefix;
  useUnsavedChanges(unsaved, { name: "the broker", pending: save.isPending });
  const movesDevices = publishing && (topic.trim() !== settings.topic || prefix.trim() !== settings.discoveryPrefix);
  // The server sends a saved password only to the broker and username it was
  // given for, so naming another asks for it again.
  const needsPassword =
    settings.passwordSet &&
    !removePassword &&
    !password &&
    (url.trim() !== settings.url || (username.trim() || undefined) !== settings.username);

  return (
    <CardBody>
      <form onSubmit={submit} className="flex flex-col gap-4">
        <FieldRow className="max-w-2xl">
          <TextField
            label="Broker"
            width={PAIR_WIDTH}
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
        </FieldRow>
        <FieldRow className="max-w-2xl">
          <TextField
            label="Username"
            width={FIELD_WIDTH}
            value={username}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setUsername(event.target.value)}
          />
          <TextField
            label="Password"
            width={FIELD_WIDTH}
            hint={needsPassword ? "Another broker or username needs the password again." : undefined}
            type="password"
            value={password}
            disabled={removePassword}
            autoComplete="new-password"
            placeholder={settings.passwordSet ? "Saved; type to replace it" : ""}
            aria-invalid={needsPassword || undefined}
            onChange={(event) => setPassword(event.target.value)}
          />
        </FieldRow>
        {settings.passwordSet && (
          <Checkbox checked={removePassword} onCheckedChange={setRemovePassword}>
            Remove the saved password
          </Checkbox>
        )}
        <FieldRow className="max-w-2xl">
          <TextField
            label="Topic"
            width={FIELD_WIDTH}
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
            width={FIELD_WIDTH}
            hint="Where Home Assistant listens for new devices."
            value={prefix}
            required
            maxLength={128}
            spellCheck={false}
            onChange={(event) => setPrefix(event.target.value)}
          />
        </FieldRow>
        {movesDevices && (
          <Notice
            tone="warn"
            title="Saving takes every display out of Home Assistant under the old topic and adds it again under the new one, as a new device."
          />
        )}
        {save.isError && <Notice tone="danger" title={messageOf(save.error)} />}
        <Actions className="justify-end">
          <Button type="submit" variant="primary" disabled={save.isPending}>
            {save.isPending ? "Saving" : publishing ? "Save" : "Save and connect"}
          </Button>
        </Actions>
      </form>
    </CardBody>
  );
}

/** Taking every display out of Home Assistant, said before it happens. */
function StopPublishing({ save }: { save: Save }) {
  const [confirming, setConfirming] = useState(false);
  // Said in the dialog, which stands over the form where the save's failures show.
  const [failure, setFailure] = useState<string>();
  return (
    <SettingRow
      title="Stop publishing"
      description="Every display leaves Home Assistant, with its entities and their history there, until a broker is saved again."
    >
      <Button
        variant="destructive"
        disabled={save.isPending}
        onClick={() => {
          setFailure(undefined);
          setConfirming(true);
        }}
      >
        Stop publishing
      </Button>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Stop publishing to Home Assistant?"
        description="Every display leaves Home Assistant, with its entities and their history there, until a broker is saved again."
        confirmLabel="Stop publishing"
        destructive
        onConfirm={() =>
          save.mutateAsync({}).then(
            () => setConfirming(false),
            (error: unknown) => setFailure(messageOf(error)),
          )
        }
      >
        {failure !== undefined && <Notice tone="danger" title={failure} />}
      </ConfirmDialog>
    </SettingRow>
  );
}
