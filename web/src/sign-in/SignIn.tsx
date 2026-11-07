import type { Account } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { useNavigate, useSearchParams } from "react-router";

import { accountQuery, client, messageOf, queryClient, signInOptionsQuery } from "../app/client";
import { Wordmark } from "../app/Wordmark";
import styles from "./SignIn.module.css";

const providerErrors: Record<string, string> = {
  refused: "The sign-in provider refused the sign-in.",
  unavailable: "The sign-in provider could not be reached. Try again in a moment.",
};

/** Signing in, or creating the first account on a new server. */
export function SignIn() {
  const options = useQuery(signInOptionsQuery);
  const [search] = useSearchParams();
  const providerError = search.get("signInError");

  return (
    <main className={styles.page}>
      <div className={styles.sheet}>
        <header className={styles.header}>
          <h1 className={styles.wordmark}>
            <Wordmark />
          </h1>
          <p className={styles.tagline}>A server for TRMNL e-paper displays.</p>
        </header>
        {options.isPending && <p className="muted">Loading</p>}
        {options.isError && <p className="error-text">{messageOf(options.error)}</p>}
        {options.data && (
          <div className={styles.forms}>
            {providerError && (
              <p className="error-text" role="alert">
                {providerErrors[providerError] ?? "Signing in did not finish. Try again."}
              </p>
            )}
            {options.data.password &&
              (options.data.firstAccount ? <FirstAccountForm /> : <PasswordForm />)}
            {options.data.oidc && (
              <div className={styles.oidc}>
                {options.data.password && <span className={styles.or}>or</span>}
                <a className="button" data-variant={options.data.password ? undefined : "primary"} href="/manage/auth/oidc">
                  Sign in with {options.data.oidc.name}
                </a>
                {options.data.firstAccount && (
                  <p className="hint">The first person to sign in creates the server's first account.</p>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </main>
  );
}

function useSignedIn() {
  const navigate = useNavigate();
  return (account: Account) => {
    queryClient.setQueryData(accountQuery.queryKey, account);
    void navigate("/");
  };
}

function PasswordForm() {
  const signedIn = useSignedIn();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const signIn = useMutation({
    mutationFn: () => client.signIn({ username, password }),
    onSuccess: (output) => signedIn(output.account),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    signIn.mutate();
  };
  return (
    <form className={styles.form} onSubmit={submit}>
      <h2 className={styles.formTitle}>Sign in</h2>
      <label className="field">
        <span>Username</span>
        <input
          className="input"
          name="username"
          autoComplete="username"
          autoCapitalize="none"
          spellCheck={false}
          required
          value={username}
          onChange={(event) => setUsername(event.target.value.toLowerCase())}
        />
      </label>
      <label className="field">
        <span>Password</span>
        <input
          className="input"
          type="password"
          name="password"
          autoComplete="current-password"
          required
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
      </label>
      {signIn.isError && (
        <p className="error-text" role="alert">
          {messageOf(signIn.error)}
        </p>
      )}
      <button className="button" data-variant="primary" type="submit" disabled={signIn.isPending}>
        {signIn.isPending ? "Signing in" : "Sign in"}
      </button>
    </form>
  );
}

function FirstAccountForm() {
  const signedIn = useSignedIn();
  const [name, setName] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const create = useMutation({
    mutationFn: () => client.createFirstAccount({ name, username, password }),
    onSuccess: (output) => signedIn(output.account),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    create.mutate();
  };
  return (
    <form className={styles.form} onSubmit={submit}>
      <h2 className={styles.formTitle}>Create the first account</h2>
      <p className="hint">
        This server has no accounts yet. The first one signs in with a password; later ones come
        from the sign-in provider, when the server has one.
      </p>
      <label className="field">
        <span>Your name</span>
        <input
          className="input"
          name="name"
          autoComplete="name"
          required
          maxLength={64}
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      <label className="field">
        <span>Username</span>
        <input
          className="input"
          name="username"
          autoComplete="username"
          autoCapitalize="none"
          spellCheck={false}
          required
          minLength={2}
          maxLength={64}
          pattern="[a-z0-9][a-z0-9._\-]{1,63}"
          title="Lowercase letters, digits, dots, dashes and underscores"
          value={username}
          onChange={(event) => setUsername(event.target.value.toLowerCase())}
        />
      </label>
      <label className="field">
        <span>Password</span>
        <input
          className="input"
          type="password"
          name="password"
          autoComplete="new-password"
          required
          minLength={10}
          maxLength={256}
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        <span className="hint">At least 10 characters.</span>
      </label>
      {create.isError && (
        <p className="error-text" role="alert">
          {messageOf(create.error)}
        </p>
      )}
      <button className="button" data-variant="primary" type="submit" disabled={create.isPending}>
        {create.isPending ? "Creating" : "Create account"}
      </button>
    </form>
  );
}
