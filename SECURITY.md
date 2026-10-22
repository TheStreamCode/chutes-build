# Security Policy

Chutes Build is a fork of
[`xai-org/grok-build`](https://github.com/xai-org/grok-build). Where to report
depends on which part is affected, and if you are not sure, report it here first.

## Supported versions

Security fixes are shipped for the latest release only.

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| Older releases | No |

## Report it here

Use GitHub's private vulnerability reporting on this repository:
<https://github.com/TheStreamCode/chutes-build/security/advisories/new>.

Please do not open a public issue for a vulnerability.

Include the affected version and platform, a description of the issue and its
impact, reproduction steps or a proof of concept, and any mitigation you already
know about. You can expect an initial acknowledgement within 7 days and a status
update within 30 days. If the report is valid, a fix is prepared privately and
disclosed in the release notes once a patched version is available. Credit is
given in the advisory unless you ask otherwise.

Anything in the fork's own surface belongs here:

- credential resolution and storage (`CHUTES_API_KEY`, `~/.chutes-build/api_key`);
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
