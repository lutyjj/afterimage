import {
  Forbidden,
  type MqttSettings,
  type UpdateMqttSettingsInput,
  type UpdateMqttSettingsOutput,
} from "@afterimage/management-client";
import { useMutation, useQuery, type UseMutationResult } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";

import { client, messageOf, queryClient } from "../app/client";
import { Section } from "../app/Section";
import { mqttQuery } from "./queries";
import styles from "./Settings.module.css";

/** The MQTT broker that carries every display to Home Assistant, which only the
 * server's administrator, its first account, reads and changes. */
export function HomeAssistant() {
  const mqtt = useQuery(mqttQuery);
  // Kept here, since the form starts afresh from what the server saved.
  const save = useMutation({
    mutationFn: (request: UpdateMqttSettingsInput) => client.updateMqttSettings(request),
    onSuccess: ({ settings }) => queryClient.setQueryData(mqttQuery.queryKey, settings),
  });

  return (
    <Section
      title="MQTT broker"
      intro="Afterimage publishes every claimed display to Home Assistant through its MQTT discovery: battery, signal, last check-in, the screen it shows, and a firmware update Home Assistant can install."
    >
      {mqtt.isPending && <p className="muted">Loading the broker</p>}
      {mqtt.isError && (
        <p className="error-text" role="alert">
          {mqtt.error instanceof Forbidden
            ? "Only the server's administrator, its first account, sees these settings."
            : messageOf(mqtt.error)}
        </p>
      )}
      {mqtt.data && <Connection settings={mqtt.data} />}
      {mqtt.data?.setByDeployment && <SetByDeployment settings={mqtt.data} />}
      {mqtt.data && !mqtt.data.setByDeployment && (
        <BrokerForm key={JSON.stringify({ ...mqtt.data, connection: undefined })} settings={mqtt.data} save={save} />
      )}
    </Section>
  );
}

function Connection({ settings }: { settings: MqttSettings }) {
  const state =
    settings.connection === "connected"
      ? `Connected to ${settings.url}`
      : settings.connection === "connecting"
        ? `Connecting to ${settings.url}`
        : settings.connection === "refused"
          ? `Refused by ${settings.url}`
          : "Not publishing";
  return (
    <dl className={styles.facts} aria-live="polite">
      <div>
        <dt>Connection</dt>
        <dd>
          {state}
          {settings.connection === "connecting" && (
            <span className="muted"> · trying again every few seconds until the broker answers</span>
          )}
          {settings.connection === "refused" && (
            <span className="muted"> · the broker answered and refused; check the username and password</span>
          )}
        </dd>
      </div>
    </dl>
  );
}

/** The deployment's broker, shown as it is: it changes where the deployment sets it. */
function SetByDeployment({ settings }: { settings: MqttSettings }) {
  return (
    <>
      <dl className={styles.facts}>
        <div>
          <dt>Username</dt>
          <dd>{settings.username ?? <span className="muted">None</span>}</dd>
        </div>
        <div>
          <dt>Password</dt>
          <dd>{settings.passwordSet ? "Set" : <span className="muted">None</span>}</dd>
        </div>
        <div>
          <dt>Topic</dt>
          <dd>{settings.topic}</dd>
        </div>
        <div>
          <dt>Discovery prefix</dt>
          <dd>{settings.discoveryPrefix}</dd>
        </div>
      </dl>
      <p className="hint">
        The deployment sets this broker through <code>AFTERIMAGE_MQTT_URL</code> and the settings beside it. Change it
        there and restart Afterimage.
      </p>
    </>
  );
}

function BrokerForm({
  settings,
  save,
}: {
  settings: MqttSettings;
  save: UseMutationResult<UpdateMqttSettingsOutput, Error, UpdateMqttSettingsInput>;
}) {
  const [url, setUrl] = useState(settings.url ?? "");
  const [username, setUsername] = useState(settings.username ?? "");
  const [password, setPassword] = useState("");
  const [removePassword, setRemovePassword] = useState(false);
  const [topic, setTopic] = useState(settings.topic);
  const [prefix, setPrefix] = useState(settings.discoveryPrefix);
  const [confirmingStop, setConfirmingStop] = useState(false);

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
    <form className={styles.form} onSubmit={submit}>
      <label className={`field ${styles.wide}`}>
        <span>Broker</span>
        <input
          className="input"
          value={url}
          required
          inputMode="url"
          autoComplete="off"
          spellCheck={false}
          placeholder="mqtt://broker.example:1883"
          onChange={(event) => setUrl(event.target.value)}
        />
        <span className="hint">
          <code>mqtt://</code> or <code>mqtts://</code> for TLS, the host and its port.
        </span>
      </label>
      <label className="field">
        <span>Username</span>
        <input
          className="input"
          value={username}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setUsername(event.target.value)}
        />
      </label>
      <label className="field">
        <span>Password</span>
        <input
          className="input"
          type="password"
          value={password}
          disabled={removePassword}
          autoComplete="new-password"
          placeholder={settings.passwordSet ? "Saved; type to replace it" : ""}
          aria-invalid={needsPassword || undefined}
          onChange={(event) => setPassword(event.target.value)}
        />
        {needsPassword && <span className="hint">Another broker or username needs the password again.</span>}
      </label>
      {settings.passwordSet && (
        <label className={`${styles.check} ${styles.wide}`}>
          <input type="checkbox" checked={removePassword} onChange={(event) => setRemovePassword(event.target.checked)} />
          <span>Remove the saved password</span>
        </label>
      )}
      <label className="field">
        <span>Topic</span>
        <input
          className="input"
          value={topic}
          required
          maxLength={64}
          pattern="[A-Za-z0-9_\-]+"
          spellCheck={false}
          onChange={(event) => setTopic(event.target.value)}
        />
      </label>
      <label className="field">
        <span>Discovery prefix</span>
        <input
          className="input"
          value={prefix}
          required
          maxLength={128}
          spellCheck={false}
          onChange={(event) => setPrefix(event.target.value)}
        />
      </label>
      <p className={`hint ${styles.wide}`}>
        {movesDevices
          ? "Saving takes every display out of Home Assistant under the old topic and adds it again under the new one, as a new device."
          : "The topic names the server at the broker and starts every display's topics; Home Assistant listens under the discovery prefix."}
      </p>

      <div className={styles.actions}>
        <button className="button" data-variant="primary" type="submit" disabled={save.isPending}>
          {save.isPending && !confirmingStop ? "Saving" : publishing ? "Save" : "Save and connect"}
        </button>
        {save.isSuccess && <span className="hint">{save.data.settings.url ? "Saved" : "Stopped"}</span>}
        {save.isError && (
          <span className="error-text" role="alert">
            {messageOf(save.error)}
          </span>
        )}
      </div>

      {publishing && (
        <div className={`${styles.stop} ${styles.wide}`}>
          {!confirmingStop ? (
            <button className="button" data-variant="danger" type="button" onClick={() => setConfirmingStop(true)}>
              Stop publishing…
            </button>
          ) : (
            <>
              <p className={`hint ${styles.consequence}`}>
                Every display leaves Home Assistant, with its entities and their history there, until a broker is
                saved again.
              </p>
              <button
                className="button"
                data-variant="danger-solid"
                type="button"
                disabled={save.isPending}
                onClick={() => save.mutate({})}
              >
                {save.isPending ? "Stopping" : "Stop publishing"}
              </button>
              <button className="button" data-variant="quiet" type="button" onClick={() => setConfirmingStop(false)}>
                Keep publishing
              </button>
            </>
          )}
        </div>
      )}
    </form>
  );
}
