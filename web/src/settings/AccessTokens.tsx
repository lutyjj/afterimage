import { Dialog } from "@base-ui/react/dialog";
import type { AccessToken } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  Button,
  Card,
  CardBody,
  CardHead,
  Checkbox,
  Chip,
  ConfirmDialog,
  CopyValue,
  Count,
  DialogCancel,
  DialogLayout,
  DialogRoot,
  DialogShell,
  Field,
  Nothing,
  SettingRow,
  SettlingRows,
  ShownOnce,
  Swap,
  TextField,
  useToast,
} from "linen";
import { Plus } from "lucide-react";
import { type FormEvent, useState } from "react";

import { client, messageOf, queryClient, serverQuery } from "../app/client";
import { ago, moment } from "../app/format";
import { tokensQuery } from "./queries";

type Created = { token: AccessToken; secret: string };

/** The personal access tokens scripts and agents act with, and how an agent
 * connects over MCP with one. */
export function AccessTokens() {
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const tokens = useQuery(tokensQuery);
  const [creating, setCreating] = useState(false);

  return (
    <>
      <Card>
        <CardHead
          title="Access tokens"
          aside={<Count>{tokens.data?.length}</Count>}
          description={
            <>
              Scripts and agents send a token instead of signing in, as{" "}
              <code className="font-mono text-xs">Authorization: Bearer</code> and the token. It acts as you until you
              revoke it; give each its own, so you can revoke one alone.
            </>
          }
        >
          <Button variant="primary" onClick={() => setCreating(true)}>
            <Plus aria-hidden="true" className="size-4" />
            New token
          </Button>
        </CardHead>
        {tokens.isPending && <Nothing>Reading tokens</Nothing>}
        {tokens.isError && (
          <CardBody>
            <p role="alert" className="text-sm text-danger">
              {messageOf(tokens.error)}
            </p>
          </CardBody>
        )}
        {tokens.data?.length === 0 && <Nothing>No tokens yet.</Nothing>}
        {/* The list says what it answers once the tokens have come, so the
            ones that load are where it starts and only a new one arrives. It
            keeps its scope through a failed refetch, which keeps the tokens. */}
        <SettlingRows scope={tokens.data === undefined ? undefined : "tokens"} marksArrivals>
          {tokens.data?.map((token) => (
            <TokenRow key={token.id} token={token} timeZone={timeZone} />
          ))}
        </SettlingRows>
      </Card>
      <Agents />
      <NewToken open={creating} onOpenChange={setCreating} />
    </>
  );
}

/** How an agent reaches the MCP endpoint: its address, and the token in its
 * `Authorization` header. */
function Agents() {
  const endpoint = `${window.location.origin}/mcp`;
  const command = `claude mcp add --transport http afterimage ${endpoint} --header "Authorization: Bearer <token>"`;
  return (
    <Card>
      <CardHead
        title="Agents"
        description="An agent that speaks MCP connects over streamable HTTP with a token, and writes screens, previews them and arranges playlists through the same API as this page."
      />
      <CardBody>
        <Field label="Address">
          <CopyValue label="MCP address" value={endpoint} />
        </Field>
        <Field
          label="Claude Code"
          hint="Put a token in place of <token>. A read-only token gives an agent everything to read and nothing to change."
          className="mb-0"
        >
          <CopyValue label="Claude Code command" value={command} />
        </Field>
      </CardBody>
    </Card>
  );
}

/** Making a token: its name and reach, then its secret, which the server shows
 * this once. The dialog turns from the one to the other in place. */
function NewToken({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [name, setName] = useState("");
  const [readOnly, setReadOnly] = useState(false);
  const [created, setCreated] = useState<Created>();
  const create = useMutation({
    mutationFn: () => client.createAccessToken({ name: name.trim(), scope: readOnly ? "read_only" : undefined }),
    onSuccess: (result) => {
      void queryClient.invalidateQueries({ queryKey: tokensQuery.queryKey });
      setCreated(result);
    },
  });
  const change = (next: boolean) => {
    // A token being made would arrive after the dialog closed, its secret
    // never shown; the dialog stays until it has.
    if (!next && create.isPending) return;
    onOpenChange(next);
    if (next) return;
    // Forgotten as the dialog closes; it keeps its words while it leaves.
    setName("");
    setReadOnly(false);
    setCreated(undefined);
    create.reset();
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (name.trim() && !create.isPending) create.mutate();
  };

  return (
    <DialogRoot open={open} onOpenChange={change}>
      <DialogShell open={open} width="30rem">
        <Swap view={created ? "secret" : "form"}>
          {created ? (
            <DialogLayout
              title={`${created.token.name} is ready`}
              actions={<Dialog.Close render={<Button variant="primary">Done</Button>} />}
            >
              <ShownOnce what="Token" label="Access token" secret={created.secret} copyLabel="Copy token">
                Give it to the script or agent that will use it.
              </ShownOnce>
            </DialogLayout>
          ) : (
            <form onSubmit={submit}>
              <DialogLayout
                title="New access token"
                description="It acts as you until you revoke it."
                actions={
                  <>
                    <DialogCancel />
                    <Button type="submit" variant="primary" disabled={!name.trim() || create.isPending}>
                      {create.isPending ? "Creating" : "Create token"}
                    </Button>
                  </>
                }
              >
                <TextField
                  label="Name"
                  hint="What will hold it, so you know which to revoke."
                  value={name}
                  required
                  maxLength={64}
                  autoFocus
                  placeholder="Backup script"
                  onChange={(event) => setName(event.target.value)}
                />
                <Checkbox checked={readOnly} onCheckedChange={setReadOnly}>
                  <span className="grid">
                    Read only
                    <span className="text-xs text-text-secondary">
                      It can read devices, screens and playlists, and change nothing.
                    </span>
                  </span>
                </Checkbox>
                {create.isError && (
                  <p role="alert" className="mt-3 text-sm text-danger">
                    {messageOf(create.error)}
                  </p>
                )}
              </DialogLayout>
            </form>
          )}
        </Swap>
      </DialogShell>
    </DialogRoot>
  );
}

function TokenRow({ token, timeZone }: { token: AccessToken; timeZone: string | undefined }) {
  const toast = useToast();
  const [confirming, setConfirming] = useState(false);
  const revoke = useMutation({
    mutationFn: () => client.revokeAccessToken({ tokenId: token.id }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: tokensQuery.queryKey });
      toast.show({ text: `${token.name} revoked` });
    },
    onError: (error) => toast.show({ text: `${token.name} was not revoked. ${messageOf(error)}`, sticky: true }),
  });

  return (
    <SettingRow
      title={
        <span className="flex flex-wrap items-center gap-2">
          {token.name}
          <Chip size="row">{token.scope === "read_only" ? "Read only" : "Full access"}</Chip>
        </span>
      }
      description={
        <>
          Created {timeZone ? moment(token.createdAt, timeZone) : ago(token.createdAt)},{" "}
          {token.lastUsedAt ? `last used ${ago(token.lastUsedAt)}` : "never used"}
        </>
      }
    >
      <Button variant="destructive" disabled={revoke.isPending} onClick={() => setConfirming(true)}>
        {revoke.isPending ? "Revoking" : "Revoke"}
      </Button>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Revoke ${token.name}?`}
        detail="Whatever uses it stops working at once. This cannot be undone."
        confirmLabel="Revoke"
        destructive
        onConfirm={() => revoke.mutate()}
      />
    </SettingRow>
  );
}
