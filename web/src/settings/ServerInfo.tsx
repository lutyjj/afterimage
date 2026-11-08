import { useQuery } from "@tanstack/react-query";
import { Card, CardBody, CardHead, Nothing, SettingRow } from "linen";

import { messageOf, serverQuery, signInOptionsQuery } from "../app/client";

/** What the deployment gives every account. It changes where the deployment
 * sets it, so here it is only shown. */
export function ServerInfo() {
  const server = useQuery(serverQuery);
  const signIn = useQuery(signInOptionsQuery).data;

  return (
    <Card>
      <CardHead title="Server" description="Set by the deployment, for every account on this server." />
      {server.isPending && <Nothing>Reading the server's settings</Nothing>}
      {server.isError && (
        <CardBody>
          <p role="alert" className="text-sm text-danger">
            {messageOf(server.error)}
          </p>
        </CardBody>
      )}
      {server.data && (
        <>
          <SettingRow title="Release">
            <span className="text-sm tabular-nums">{server.data.version}</span>
          </SettingRow>
          <SettingRow title="Time zone" description="Schedules keep its time, and screens show dates in it.">
            <span className="text-sm">{server.data.timeZone}</span>
          </SettingRow>
          <SettingRow
            title="Secrets"
            description={
              <>
                Data source requests read them as <code className="font-mono text-xs">secrets.NAME</code>. Every account
                may send them in its requests.
              </>
            }
          >
            {server.data.secretNames.length === 0 ? (
              <span className="text-sm text-text-muted">None</span>
            ) : (
              <span className="flex flex-wrap justify-end gap-1">
                {server.data.secretNames.map((name) => (
                  <code key={name} className="rounded-input bg-sunken px-1.5 py-0.5 font-mono text-xs">
                    {name}
                  </code>
                ))}
              </span>
            )}
          </SettingRow>
          <SettingRow title="Sign-in">
            <span className="text-sm">
              {[signIn?.oidc && `Through ${signIn.oidc.name}`, signIn?.password && "With a password"]
                .filter(Boolean)
                .join(", ")}
            </span>
          </SettingRow>
        </>
      )}
    </Card>
  );
}
