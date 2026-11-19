import { useQuery } from "@tanstack/react-query";
import { Card, CardBody, CardHead, Fact, Facts, Nothing, SettingNote } from "linen";

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
          <CardBody>
            <Facts>
              <Fact name="Release">
                <span className="tabular-nums">{server.data.version}</span>
              </Fact>
              <Fact name="Time zone">{server.data.timeZone}</Fact>
              <Fact name="Secrets">
                {server.data.secretNames.length === 0 ? (
                  <span className="text-text-muted">None</span>
                ) : (
                  <span className="flex flex-wrap justify-end gap-1">
                    {server.data.secretNames.map((name) => (
                      <code key={name} className="rounded-input bg-sunken px-1.5 py-0.5 font-mono text-xs">
                        {name}
                      </code>
                    ))}
                  </span>
                )}
              </Fact>
              <Fact name="Sign-in">
                {[signIn?.oidc && `Through ${signIn.oidc.name}`, signIn?.password && "With a password"]
                  .filter(Boolean)
                  .join(", ")}
              </Fact>
            </Facts>
          </CardBody>
          <SettingNote>
            Schedules keep the time zone, and screens show dates in it. Data source requests read a secret as{" "}
            <code className="font-mono">secrets.NAME</code>, and every account may send them in its requests.
          </SettingNote>
        </>
      )}
    </Card>
  );
}
