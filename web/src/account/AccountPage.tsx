import type { AccessToken } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Check, Copy } from "lucide-react";
import { useState, type FormEvent } from "react";

import { accountQuery, client, messageOf, queryClient, serverQuery } from "../app/client";
import { ago, moment } from "../app/format";
import styles from "./AccountPage.module.css";
import { tokensQuery } from "./queries";

/** Who is signed in, and the personal API tokens scripts and agents act with. */
export function AccountPage() {
  const account = useQuery(accountQuery).data;
  const timeZone = useQuery(serverQuery).data?.timeZone;
  const tokens = useQuery(tokensQuery);
  const [created, setCreated] = useState<{ token: AccessToken; secret: string } | undefined>();

  return (
    <div className={styles.page}>
      <header className={styles.head}>
        <h1 className={styles.heading}>{account?.name ?? "Account"}</h1>
        <p className="annotation">
          {account?.username ? `Signs in as ${account.username}` : "Signs in through single sign-on"}
        </p>
      </header>

      <section className={styles.section} aria-labelledby="tokens-title">
        <div className={styles.sectionHead}>
          <h2 id="tokens-title" className={styles.title}>
            API tokens
          </h2>
          <p className="hint">
            Scripts and agents send a token instead of signing in, as <code>Authorization: Bearer</code> and the
            token. It acts as you until you revoke it. Give each script its own, so that you can revoke one alone.
          </p>
        </div>

        {created ? (
          <ShownOnce created={created} onDone={() => setCreated(undefined)} />
        ) : (
          <CreateToken onCreated={setCreated} />
        )}

        {tokens.isPending && <p className="annotation">Loading tokens</p>}
        {tokens.isError && (
          <p className="error-text" role="alert">
            {messageOf(tokens.error)}
          </p>
        )}
        {tokens.data?.length === 0 && <p className="hint">No tokens yet.</p>}
        {tokens.data && tokens.data.length > 0 && (
          <ul className={styles.tokens}>
            {tokens.data.map((token) => (
              <TokenRow key={token.id} token={token} timeZone={timeZone} />
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function CreateToken({ onCreated }: { onCreated: (created: { token: AccessToken; secret: string }) => void }) {
  const [name, setName] = useState("");
  const [readOnly, setReadOnly] = useState(false);
  const create = useMutation({
    mutationFn: () =>
      client.createAccessToken({ name: name.trim(), scope: readOnly ? "read_only" : undefined }),
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
      <label className={styles.readOnly}>
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
function ShownOnce({ created, onDone }: { created: { token: AccessToken; secret: string }; onDone: () => void }) {
  const [copied, setCopied] = useState(false);
  const [copyFailed, setCopyFailed] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(created.secret);
      setCopied(true);
      setCopyFailed(false);
    } catch {
      setCopyFailed(true);
    }
  };

  return (
    <div className={styles.shownOnce} role="status">
      <p>
        <strong>{created.token.name}</strong> is ready. Copy its token now: Afterimage keeps only a fingerprint of it
        and cannot show it again.
      </p>
      <div className={styles.secretRow}>
        <code className={styles.secret} onFocus={(event) => window.getSelection()?.selectAllChildren(event.currentTarget)} tabIndex={0}>
          {created.secret}
        </code>
        <button className="button" type="button" onClick={() => void copy()}>
          {copied ? <Check size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      {copyFailed && (
        <p className="error-text" role="alert">
          The browser did not allow copying. Select the token and copy it yourself.
        </p>
      )}
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
        <span className={styles.tokenName}>{token.name}</span>
        <span className={styles.scope}>{token.scope === "read_only" ? "Read only" : "Full access"}</span>
        <span className={styles.when}>
          Created {timeZone ? moment(token.createdAt, timeZone) : ago(token.createdAt)}
          {" · "}
          {token.lastUsedAt ? `last used ${ago(token.lastUsedAt)}` : "never used"}
        </span>
      </div>
      {!confirming ? (
        <button className="button" data-variant="danger" type="button" onClick={() => setConfirming(true)}>
          Revoke…
        </button>
      ) : (
        <div className={styles.confirm}>
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
