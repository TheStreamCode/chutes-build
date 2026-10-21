# Contributing

## Scope

This repository is maintained by Michael Gasperini (Mikesoft). Mikesoft develops this software
internally, and external pull requests and unsolicited patches are not accepted. The public tree is
published for source transparency and local builds under the Apache License, Version 2.0.

Bug reports and focused proposals are welcome through GitHub Issues. Support questions belong in the
channel described in [`SUPPORT.md`](SUPPORT.md). Security issues must follow [`SECURITY.md`](SECURITY.md)
and must not be reported publicly.

## Reporting a bug

Open a bug report and include:

- what you expected and what happened;
- the exact version (`chutes-build --version` or the source revision);
- operating system, terminal, and the Chutes model or endpoint involved;
- the smallest reproduction you can share, with sanitized logs and error text.

Remove credentials, prompts, private source code and personal data. Reports without version
information cannot be reproduced and may be closed.

## Proposing a change

Open an issue describing the problem before any implementation. Pull requests opened from outside
the maintainer are closed without review.

## Conventions

- Conventional Commits: `type(scope): summary`, English, imperative mood.
- One logical change per commit and per pull request.
- Run the checks in the README development section before asking for a review.
- Keep the Apache-2.0 attribution intact: `NOTICE`, `THIRD-PARTY-NOTICES` and the in-tree
  third-party notices are not rewritten by hand.
- Generated or inherited files (`Cargo.lock`, the root `Cargo.toml`, `SOURCE_REV`) are regenerated
  through their owning process, never edited directly.
- Do not merge `upstream/main`; port upstream changes by hand as described in
  [`docs/upstream-sync.md`](docs/upstream-sync.md).

## Security reports

Report security issues through the process described in [`SECURITY.md`](SECURITY.md). Do not open a
public issue for vulnerabilities.

## Licensing of this source

By downloading or using this source, you agree that your use is governed by the Apache License,
Version 2.0 (see [`LICENSE`](LICENSE)). No contributor license agreement is offered because external
contributions are not accepted.
