import type { AccessToken } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Check, Copy } from "lucide-react";
import { useState, type FormEvent } from "react";

import { client, messageOf, queryClient, serverQuery } from "../app/client";
import { ago, moment } from "../app/format";
import { Section } from "../app/Section";
import { tokensQuery } from "./queries";
import styles from "./Settings.module.css";

type Created = { token: AccessToken; secret: string };

/** The personal access tokens scripts and agents act with, and how an agent
 * connects over MCP with one. */
export function AccessTokens() {
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const tokens = useQuery(tokensQuery);
  const [created, setCreated] = useState<Created | undefined>();

  return (
    <>
      <Section
        title="Access tokens"
        intro={
          <>
            Scripts and agents send a token instead of signing in, as <code>Authorization: Bearer</code> and the
            token. It acts as you until you revoke it; give each its own, so you can revoke one alone.
          </>
        }
      >
        {created ? (
          <ShownOnce created={created} onDone={() => setCreated(undefined)} />
        ) : (
          <CreateToken onCreated={setCreated} />
        )}
        {tokens.isPending && <p className="muted">Loading tokens</p>}
        {tokens.isError && (
          <p className="error-text" role="alert">
            {messageOf(tokens.error)}
          </p>
        )}
        {tokens.data?.length === 0 && <p className="muted">No tokens yet.</p>}
        {tokens.data && tokens.data.length > 0 && (
          <ul className={styles.tokens}>
            {tokens.data.map((token) => (
              <TokenRow key={token.id} token={token} timeZone={timeZone} />
            ))}
          </ul>
        )}
      </Section>
      <Agents secret={created?.secret} />
    </>
  );
}

/** How an agent reaches the MCP endpoint: its address, and the token in its
 * `Authorization` header. */
function Agents({ secret }: { secret: string | undefined }) {
  const endpoint = `${window.location.origin}/mcp`;
  const command = `claude mcp add --transport http afterimage ${endpoint} --header "Authorization: Bearer ${secret ?? "<token>"}"`;
  return (
    <Section
      title="Agents"
      intro="An agent that speaks MCP writes screens, previews them and arranges playlists with a token, through the same API as this page."
    >
      <dl className={styles.facts}>
        <div>
          <dt>Address</dt>
          <dd>
            <CopyText text={endpoint} />
          </dd>
        </div>
        <div>
          <dt>Transport</dt>
          <dd>Streamable HTTP, with the token as a bearer token</dd>
        </div>
        <div>
          <dt>Claude Code</dt>
          <dd>
            <CopyText text={command} />
          </dd>
        </div>
      </dl>
      <p className="hint">A read-only token gives an agent everything to read and nothing to change.</p>
    </Section>
  );
}

/** A value to copy whole. Where the browser refuses the clipboard, as a page
 * served without TLS on a network address is refused, it says so: the value
 * may be a secret shown once. */
function CopyText({ text }: { text: string }) {
  const [copied, setCopied] = useState<boolean | "failed">(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied("failed");
    }
  };
  return (
    <span className={styles.copy}>
      <span className={styles.copyText}>
        <code tabIndex={0} onFocus={(event) => window.getSelection()?.selectAllChildren(event.currentTarget)}>
          {text}
        </code>
        <button type="button" className="icon-button" onClick={() => void copy()} aria-label="Copy" title="Copy">
          {copied === true ? <Check size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}
        </button>
      </span>
      {copied === "failed" && (
        <span className="error-text" role="alert">
          The browser did not allow copying. Select the text and copy it yourself.
        </span>
      )}
    </span>
  );
}

function CreateToken({ onCreated }: { onCreated: (created: Created) => void }) {
  const [name, setName] = useState("");
  const [readOnly, setReadOnly] = useState(false);
  const create = useMutation({
    mutationFn: () => client.createAccessToken({ name: name.trim(), scope: readOnly ? "read_only" : undefined }),
    onSuccess: (created) => {
      void queryClient.invalidateQueries({ queryKey: tokensQuery.queryKey });
      setName("");
      setReadOnly(false);
      onCreated(created);
    },
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    create.mutate();
  };

  return (
    <form className={styles.create} onSubmit={submit}>
      <label className="field">
        <span>Name</span>
        <input
          className="input"
          value={name}
          required
          maxLength={64}
          placeholder="Provisioning script"
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      <label className={styles.check}>
        <input type="checkbox" checked={readOnly} onChange={(event) => setReadOnly(event.target.checked)} />
        <span>
          Read only
          <span className="hint">It can read devices, screens and playlists, and change nothing.</span>
        </span>
      </label>
      <div className={styles.actions}>
        <button className="button" data-variant="primary" type="submit" disabled={!name.trim() || create.isPending}>
          {create.isPending ? "Creating" : "Create token"}
        </button>
        {create.isError && (
          <span className="error-text" role="alert">
            {messageOf(create.error)}
          </span>
        )}
      </div>
    </form>
  );
}

/** The new token's secret, which the server never shows again. */
function ShownOnce({ created, onDone }: { created: Created; onDone: () => void }) {
  return (
    <div className={styles.shownOnce} role="status">
      <p>
        <strong>{created.token.name}</strong> is ready. Copy its token now: Afterimage keeps only a fingerprint of it
        and cannot show it again.
      </p>
      <CopyText text={created.secret} />
      <div className={styles.actions}>
        <button className="button" data-variant="primary" type="button" onClick={onDone}>
          Done
        </button>
      </div>
    </div>
  );
}

function TokenRow({ token, timeZone }: { token: AccessToken; timeZone: string | undefined }) {
  const [confirming, setConfirming] = useState(false);
  const revoke = useMutation({
    mutationFn: () => client.revokeAccessToken({ tokenId: token.id }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: tokensQuery.queryKey }),
  });

  return (
    <li className={styles.token}>
      <div className={styles.tokenFacts}>
        <span className={styles.tokenName}>
          {token.name}
          <span className={styles.tag}>{token.scope === "read_only" ? "Read only" : "Full access"}</span>
        </span>
        <span className="muted">
          Created {timeZone ? moment(token.createdAt, timeZone) : ago(token.createdAt)},{" "}
          {token.lastUsedAt ? `last used ${ago(token.lastUsedAt)}` : "never used"}
        </span>
      </div>
      {!confirming ? (
        <button className="button" data-variant="danger" type="button" onClick={() => setConfirming(true)}>
          Revoke…
        </button>
      ) : (
        <div className={styles.actions}>
          <span className="hint">Whatever uses it stops working at once.</span>
          <button
            className="button"
            data-variant="danger-solid"
            type="button"
            disabled={revoke.isPending}
            onClick={() => revoke.mutate()}
          >
            {revoke.isPending ? "Revoking" : "Revoke"}
          </button>
          <button className="button" data-variant="quiet" type="button" onClick={() => setConfirming(false)}>
            Keep it
          </button>
        </div>
      )}
      {revoke.isError && (
        <p className="error-text" role="alert">
          {messageOf(revoke.error)}
        </p>
      )}
    </li>
  );
}
