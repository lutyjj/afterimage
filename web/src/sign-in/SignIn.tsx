import type { Account } from "@afterimage/management-client";
import { useMutation, useQuery } from "@tanstack/react-query";
import { arrive, Button, buttonClass, classes, TextField } from "linen";
import { m } from "motion/react";
import { useState, type FormEvent } from "react";
import { useNavigate, useSearchParams } from "react-router";

import { accountQuery, client, messageOf, queryClient, signInOptionsQuery } from "../app/client";
import { Wordmark } from "../app/Wordmark";

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
    <main className="grid min-h-dvh place-items-center px-4 py-8">
      <m.div
        {...arrive()}
        className="grid w-full max-w-sm gap-8 rounded-dialog border border-border bg-surface p-6 sm:p-8"
      >
        <header className="grid gap-2">
          <h1 className="text-lg">
            <Wordmark />
          </h1>
          <p className="text-sm text-text-secondary">A server for TRMNL e-paper displays.</p>
        </header>
        {options.isPending && <p className="text-sm text-text-muted">Reading the server</p>}
        {options.isError && (
          <p role="alert" className="text-sm text-danger">
            {messageOf(options.error)}
          </p>
        )}
        {options.data && (
          <div className="grid gap-6">
            {providerError && (
              <p role="alert" className="text-sm text-danger">
                {providerErrors[providerError] ?? "Signing in did not finish. Try again."}
              </p>
            )}
            {options.data.password && (options.data.firstAccount ? <FirstAccountForm /> : <PasswordForm />)}
            {options.data.oidc && (
              <div className="grid gap-3">
                {options.data.password && (
                  <span className="flex items-center gap-3 text-sm text-text-muted before:h-px before:flex-1 before:bg-border after:h-px after:flex-1 after:bg-border">
                    or
                  </span>
                )}
                <a
                  className={classes(buttonClass(options.data.password ? "default" : "primary"), "w-full")}
                  href="/manage/auth/oidc"
                >
                  Sign in with {options.data.oidc.name}
                </a>
                {options.data.firstAccount && (
                  <p className="text-xs text-text-secondary">
                    The first person to sign in creates the server's first account.
                  </p>
                )}
              </div>
            )}
          </div>
        )}
      </m.div>
    </main>
  );
}

function useSignedIn() {
  const navigate = useNavigate();
  return (account: Account) => {
    queryClient.setQueryData(accountQuery.queryKey, account);
    void navigate("/devices");
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
    <form className="grid" onSubmit={submit}>
      <h2 className="mb-4 text-md font-semibold">Sign in</h2>
      <TextField
        label="Username"
        name="username"
        autoComplete="username"
        autoCapitalize="none"
        spellCheck={false}
        required
        value={username}
        onChange={(event) => setUsername(event.target.value.toLowerCase())}
      />
      <TextField
        label="Password"
        type="password"
        name="password"
        autoComplete="current-password"
        required
        value={password}
        onChange={(event) => setPassword(event.target.value)}
      />
      {signIn.isError && (
        <p role="alert" className="mb-4 text-sm text-danger">
          {messageOf(signIn.error)}
        </p>
      )}
      <Button type="submit" variant="primary" className="w-full" disabled={signIn.isPending}>
        {signIn.isPending ? "Signing in" : "Sign in"}
      </Button>
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
    <form className="grid" onSubmit={submit}>
      <h2 className="text-md font-semibold">Create the first account</h2>
      <p className="mt-1 mb-4 text-sm text-text-secondary">
        This server has no accounts yet. The first one signs in with a password; later ones come from the sign-in
        provider, when the server has one.
      </p>
      <TextField
        label="Your name"
        name="name"
        autoComplete="name"
        required
        maxLength={64}
        value={name}
        onChange={(event) => setName(event.target.value)}
      />
      <TextField
        label="Username"
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
      <TextField
        label="Password"
        hint="At least 10 characters."
        type="password"
        name="password"
        autoComplete="new-password"
        required
        minLength={10}
        maxLength={256}
        value={password}
        onChange={(event) => setPassword(event.target.value)}
      />
      {create.isError && (
        <p role="alert" className="mb-4 text-sm text-danger">
          {messageOf(create.error)}
        </p>
      )}
      <Button type="submit" variant="primary" className="w-full" disabled={create.isPending}>
        {create.isPending ? "Creating" : "Create account"}
      </Button>
    </form>
  );
}
