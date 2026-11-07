import { useQuery } from "@tanstack/react-query";

import { messageOf, serverQuery, signInOptionsQuery } from "../app/client";
import { Row, Rows, Section } from "../app/Section";
import styles from "./Settings.module.css";

/** What the deployment gives every account. It changes where the deployment
 * sets it, so here it is only shown. */
export function ServerInfo() {
  const server = useQuery(serverQuery);
  const signIn = useQuery(signInOptionsQuery).data;

  if (server.isPending) {
    return <p className="muted">Loading the server's settings</p>;
  }
  if (server.isError) {
    return <p className="error-text">{messageOf(server.error)}</p>;
  }
  const data = server.data;
  return (
    <Section title="Server" intro="Set by the deployment, for every account on this server.">
      <Rows>
        <Row label="Release">{data.version}</Row>
        <Row label="Time zone" description="Playlists keep its time, and screens show dates in it.">
          {data.timeZone}
        </Row>
        <Row
          label="Secrets"
          description={
            <>
              Data source requests read them as <code>secrets.NAME</code>. Every account may send them in its
              requests.
            </>
          }
        >
          {data.secretNames.length === 0 ? (
            <span className="muted">None</span>
          ) : (
            <span className={styles.names}>
              {data.secretNames.map((name) => (
                <code key={name}>{name}</code>
              ))}
            </span>
          )}
        </Row>
        <Row label="Sign-in">
          {[signIn?.oidc && `Through ${signIn.oidc.name}`, signIn?.password && "With a password"]
            .filter(Boolean)
            .join(", ")}
        </Row>
      </Rows>
    </Section>
  );
}
