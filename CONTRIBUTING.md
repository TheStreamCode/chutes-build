# Contributing

Chutes Build is a fork of [`xai-org/grok-build`](https://github.com/xai-org/grok-build),
maintained by [TheStreamCode](https://github.com/TheStreamCode) under the Apache
License, Version 2.0 (see [`LICENSE`](LICENSE)).

**Before opening a pull request, please open an issue first.** Most of this tree
is upstream's code, and it is kept close to upstream on purpose so their fixes
arrive by merge rather than by hand — see
[`docs/upstream-sync.md`](docs/upstream-sync.md). A change that diverges from
upstream without needing to makes every future sync harder, so it is worth
agreeing on the approach before the work.

Changes that fit this fork's purpose are the Chutes ecosystem integration, the
privacy posture, and the platform gaps upstream leaves open (Windows in
particular). A fix that belongs upstream is better sent upstream: it reaches more
people, and it comes back here by merge.

Anything touching credentials, endpoints, or the privacy constants in
`chutes-build-core::product` needs the reasoning in the PR description, not just
the diff.

## Security reports

Please report security issues through the process described in
[`SECURITY.md`](SECURITY.md). Do not open a public issue for vulnerabilities.

## Licensing of this source

By downloading or using this source, you agree that your use is governed by the
Apache License, Version 2.0. Contributions are accepted under the same licence;
there is no separate contributor licence agreement.
