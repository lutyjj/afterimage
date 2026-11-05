# AGENTS.md

Afterimage is a self-hosted server for TRMNL e-paper displays. [The design](docs/design.md) owns its scope, principles, components and open questions.

## Maintenance rules

- **Treat this project as greenfield. No backwards compatibility:** no
  deprecated paths, compatibility shims, or transitional implementations.
  Rewrite a unit when extending it would make it harder to maintain.
- Complete replacements in one change: remove obsolete code, configuration,
  callers, tests, and documentation. Keep one implementation of each contract.
- Database migrations are the exception: never edit or delete a migration that
  has reached `main`.
- Follow the [design principles](docs/design.md#principles): implement TRMNL's
  device API as published, generate from TRMNL's published sources, and keep
  device facts in data, never in code paths.
- Prefer an established dependency when it removes substantial custom machinery.
  Evaluate its maintenance, integration cost, and behavior before adding it.
- Preserve unrelated edits. Do not run separate processes that concurrently
  mutate shared generated output or installed dependencies. Run cleanup
  separately from builds and checks.
- Write for a stranger reading the public repository. Nothing in the tree,
  commit messages, or issues may describe the maintainer's own environment,
  devices, or people: hostnames, addresses, paths, MAC addresses, access keys,
  deployment details, names, credentials. Use reserved example domains
  (`example.test`, `afterimage.example`), invented names and synthetic fixtures.
  Every commit must be fit for publication; read the diff with that reader in
  mind before committing.

## Contract and code

For API changes, update `model/smithy/` first, then regenerate and implement.
Generated output is never edited by hand. Each project builds on its own and
depends on the others only through code generated from the model, except that
any Rust project may use the shared library `crates/trmnl`.

The flake owns the toolchains; do not install unmanaged versions on the host.
The [README](README.md#development) lists the commands.

## Spikes and milestones

- A spike answers one design question quickly. It lives on a `spike/<topic>`
  branch with a README that records its method and measurements, and it is
  deleted once its answer is in the design.
- Code that others will build on gets an independent review before it lands:
  each major milestone, such as the first contract, the device API, or the
  renderer. Small changes and spikes do not need one.
- Changes proposed to upstream projects (TRMNL's firmware, framework or
  libraries) follow the upstream's own conventions, stand on public evidence,
  and are published only with the maintainer's approval.

## Verification and completion

- Run checks that exercise the changed behavior. Documentation-only edits need
  a read against the prose rules below and a check that every link and path
  they name exists.
- Relevant checks must pass before declaring the work complete. Report the
  commands and results, including failures or checks that could not run.
- Add tests where they protect meaningful behavior. Assert observable contracts
  with independently derived expectations. Exact strings and values are valid
  when they are part of the contract; avoid incidental implementation details.
- Tests must own their fixtures and control time-dependent inputs.
- Visually verify UI and rendered screens, in the running app or on a device.

## Collaboration

Use Conventional Comments for reviews: `label (decoration): subject`, such as
`issue: ...` or `suggestion (non-blocking): ...`.
Commit messages use `type(scope): imperative subject`.

Prose in documentation and comments states what is true now. No em dashes, no
attribution trailers, filler, or change-history narration; history belongs in
commit messages.

Record durable lessons in their owning code, automation, or documentation.
Add a rule here only when it applies across the repository and fills a gap;
revise an existing rule when it already covers the lesson.
