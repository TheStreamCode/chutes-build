# Security Policy

Chutes Build is a fork of
[`xai-org/grok-build`](https://github.com/xai-org/grok-build). Where to report
depends on which part is affected, and if you are not sure, report it here first.

## Report it here

Use GitHub's private vulnerability reporting on this repository:
<https://github.com/TheStreamCode/chutes-build/security/advisories/new>.

Please do not open a public issue for a vulnerability.

Anything in the fork's own surface belongs here:

- credential resolution and storage (`CHUTES_API_KEY`, `auth.json`, the OAuth
  flow against the Chutes IdP);
- which endpoints may receive a credential (`chutes-build-core::endpoint_policy`);
- the plugin trust boundary, permission rules, and the sandbox;
- the Chutes tools — media generation, OCR, the isolated browser, Context7;
- memory storage and the secret filter.

## Report it upstream too

If the flaw is in code inherited from upstream and reproduces on Grok Build, it
affects their users as well: please also report it through their process. A fix
landing upstream reaches more people, and it comes back here by merge.

## What this build does not do

Some classes of report do not apply, because the behaviour is compiled out rather
than merely disabled: telemetry, remote error reporting, session sharing, remote
workspace exposure, trace upload, and self-update. See
[`PRIVACY.md`](PRIVACY.md) and `chutes-build-core::product`. If you find one of
them active, that *is* a vulnerability and we want to hear about it.
