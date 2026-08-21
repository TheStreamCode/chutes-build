# Upstream synchronization

Chutes Build tracks `xai-org/grok-build`. As of 1.0.3 the relationship is a
**manual port**: read upstream's diff, decide per area, and carry across what is
worth carrying. Never `git merge upstream/main`.

> [!IMPORTANT]
> This reverses the model 1.0.0 adopted, and the reversal was paid for. Merging
> means taking upstream's side wherever there is no conflict, and "no conflict"
> is exactly where a fork's identity lives: the same file, quietly replaced. The
> 1.0.0 merge shipped Grok's wordmark on the splash, upstream's flag names in
> `--help`, OAuth scopes the Chutes IdP rejects so sign-in could not complete, and
> a single hardcoded context window for all thirteen models. Every one of those
> compiled and passed a green gate. Three releases went to fixing them.
>
> Upstream publishes no releases and no tags — its commits are squashed drops
> named "Synced from monorepo", and its product changelog lives at
> `x.ai/build/changelog`, off GitHub. So there is no version to "take": there is a
> diff to read.

## Why this changed

The old procedure was "port selectively, never merge upstream wholesale". It
was followed, and it still produced a fork that drifted: review records read
"nothing safe to port in isolation" and "several candidates identified, none
ported yet", so `.github/upstream.json` recorded a reviewed baseline of 0.2.117
while the retained crates were still at the 2026-07-17 fork point — 2075 files
and roughly 440k lines behind. Every individual port then had to be grafted into
files that were thousands of lines stale, which is why so many of them found our
copy *older* than the baseline rather than merely different.

Cherry-picking cannot converge on a project that adds 94k lines in five days.
So 1.0.0 re-based onto upstream's tree and re-applied the Chutes layer on top.
That layer is thin — 102 files exist only here, and only 26 consume
`chutes-build-core` — which is what makes the merge model viable.

## The procedure

```powershell
git fetch upstream main
git diff --stat <last-ported-ref>..upstream/main    # what moved, and where
```

Group the diff by area and take one area per commit, so each can be reverted on
its own. Three areas need judgement rather than a decision rule:

1. **The 26 seams** — the files that consume `chutes-build-core`: the routing
   and endpoint policy in `xai-grok-sampler/src/client.rs`, the agent config /
   init / models trio, the privacy and kill-switch call sites in
   `xai-grok-shell/src/extensions/`, `leader/server.rs`, the session and pager
   surfaces, and `xai-grok-memory/src/storage.rs`. Resolve these by hand, every
   time. They are the product.
2. **Branding** — re-run `python scripts/rebrand.py --apply`, then
   `--check`. The script carries an explicit map and fails on any forbidden
   token left outside its allowlist. It also reports four ambiguous token
   families it will never rewrite; `scripts/rebrand_files.json` holds the
   per-file decisions and can be regenerated.
3. **Deliberate divergences** — listed below.

Everywhere else, judge the hunk. "Not in one of those three categories" is not a
licence to take upstream's version — that reasoning is what shipped the wrong
wordmark, since a logo file is in no category and conflicts with nothing. Take a
change because it is worth having, not because nothing objected.

Then — and this step is not optional, it is where the 1.0.0 re-base leaked —
run the seam sweep against the release you are porting from:

```powershell
python scripts/seam_sweep.py --base <previous-release-ref>
```

A clean port and a green gate do **not** mean the seams survived: a constant
whose value came across from upstream compiles fine and its tests assert against
the constant. See the sweep section in the re-base record below for what that
cost.

The sweep reads source. It does not read **assets**, and 1.0.0 shipped upstream's
Grok wordmark because both logo files came across untouched — invisible to the
compiler, to the tests and to the sweep, and visible to every user at startup.
After a port, compare the asset tree too:

```powershell
git diff --stat upstream/main -- '**/assets/**'   # anything identical is suspect
```

A test now pins the wordmark's hash. Nothing pins the rest.

Finish with the behavioural checks the gate cannot do: `--version`, `--help`,
`models`, `du`, and a confirmation that nothing was written outside
`$CHUTES_BUILD_HOME`.

## Non-negotiables during a port

- Never `git add -A` after running the test suite. It has committed live API
  keys to this branch once already: a test that shells out to `env` writes the
  whole environment somewhere, and on Windows that somewhere turned out to be the
  crate root. Stage deliberately, and read what you staged. See the leak record
  below.
- No telemetry, remote error reporting, automatic upload, upstream session
  sharing, remote workspace exposure or phone-home updates. These are enforced
  by the compile-time constants in `crates/chutes-build-core/src/product.rs`
  consumed at their call sites — **not** by deleting the crates, which would
  make every future diff unreadable.
- Never send ambient Chutes credentials to an endpoint that has not passed
  `chutes-build-core::endpoint_policy`. Env-overridable base URLs
  (`CHUTES_ROUTER_BASE_URL` and friends) are the specific hazard.
- Hostnames are logic, not branding. `sampling/error.rs` matches the literal
  `grok.com/supergrok` on the way *in*, to recognise an upsell the upstream
  server sends and suppress it for API-key users. Rewriting that string passes
  every test and silently starts showing team users a personal-subscription
  pitch.
- Rust identifiers are not renamed. `grok_home()`, `default_grok_home()` and the
  `xai-grok-*` crate names stay; only the strings they return become Chutes.
  Renaming them makes upstream diffs unreadable for no gain.
- The model catalogue is read, not assumed. `llm.chutes.ai/v1/models` publishes
  `context_length`, `max_output_length`, `supported_features` and
  `input_modalities` per model; upstream's parser looks for none of those names.
  If a port touches `parse_remote_model_value`, check those fallbacks survived.

## Deliberate divergences to preserve

- `crates/chutes-build-core/` and `xai-grok-tools/src/implementations/chutes/`
  are ours outright.
- Chutes routing: the virtual `model-router` model, `CHUTES_FALLBACK_MODELS`,
  and the per-family reasoning normalisation.
- `CHUTES_EXTRA_CA_BUNDLE` sits outside the `CHUTES_BUILD_*` family on purpose:
  it configures the transport, not the product shell.
- `cpk_` in the secret redactor, and the memory privacy filter.
- The `chutesnight` / `chutesday` themes replace upstream's.
- A vendored `protoc` fallback in `find_protoc`, because `bin/protoc` is a
  DotSlash wrapper Windows cannot execute from a build script.
- The endpoint table in `xai-grok-env`: the relay and gateway WebSocket URLs are
  loopback sentinels, not upstream hosts. Chutes Build has no vendor relay and no
  cloud-sandbox control plane, so those paths must fail closed rather than
  resolve.
- `LEGACY_AUTH_SCOPE` / `LEGACY_SCOPE` are `disabled::legacy-auth`: the pre-OIDC
  relay login is disabled by pointing its scope at a key no server can mint,
  rather than by deleting the code path.
- The changelog fetch and the update check point at a closed loopback port. The
  callers stay so merges stay clean; the requests fail locally and instantly.
- `CodingDataSharingLock::ProductPolicy` and the privacy banner's floor: the
  coding-data row is locked with a reason instead of removed, and the banner
  never renders. The extension behind them already refuses the call — this is so
  the product never *offers* what it will then refuse.

## Landing this on `main`, and why it constrains branch protection

> [!NOTE]
> **Superseded by the manual-port model at the top of this file.** This section
> reasons about merging `upstream/main` into a branch and squashing onto `main`,
> which is no longer how syncs are done. It is kept because the constraint it
> describes is real and would return the moment anyone reached for a merge again:
> `main` forbids non-linear history, so a merge cannot land there, and a squash
> keeps upstream's commits out of the history the next merge base is computed
> from. A manual port has neither problem — it produces ordinary commits.

`main` currently forbids non-linear history. That rule and the merge model above
are incompatible, and the incompatibility is not cosmetic.

A squash-merge of an upstream sync produces the right *tree* but keeps
upstream's commits out of `main`'s *history*. The next `git merge upstream/main`
would then still compute its merge base at the original 2026-07-17 fork point
and replay the entire divergence as conflicts — which is exactly the failure
this re-base exists to end. Squashing works once; it does not compose.

For the merge model to hold, `main` has to accept merge commits from
`upstream/main`. That is a repository-policy decision, not a code one, and it
should be made before the first sync lands. If the linear-history rule is kept
instead, the honest thing is to say so in this document and go back to
cherry-picking with eyes open, rather than discover it at the next sync.

### What actually happened, 2026-08-09

The linear-history rule was kept, and 1.0.0 landed as **one squash commit**
(`2ece18b`) whose parent is the previous `main` and whose tree is the re-based tree.
Relaxing the rule to push a merge was the other option and was not taken.

So the warning above now applies to this repository as it stands: `main`'s history
does not contain upstream's commits, and `git merge upstream/main` run *on `main`*
would compute its base at the 2026-07-17 fork point and replay everything as
conflicts.

**The model still composes, on one condition: never merge upstream into `main`.**
Merge it into a branch that carries upstream's history, and let `main` receive the
squash:

```sh
# The integration branch already contains upstream's history. Do not delete it.
git checkout rebase/upstream-1.0.0
git merge upstream/main          # merge base is correct here
#   ... resolve the Chutes seams, run the gate, then land the tree on main:
tree=$(git rev-parse rebase/upstream-1.0.0^{tree})
commit=$(git commit-tree "$tree" -p origin/main -F message.txt)
git push origin "$commit:refs/heads/main"
```

`rebase/upstream-1.0.0` is pushed and is that branch today. Renaming it to something
durable — `integration/upstream` — would say what it is for, but it must not be
deleted as a stale feature branch: it is the only place upstream's history is
reachable from, and deleting it puts the next sync back at the fork point.

Two things follow. `main` is a squash-only release branch, and its commits will not
correspond one-to-one with the work; `CHANGELOG.md` and this file are the record
instead. And the choice is still open: relaxing linear history removes the whole
dance, at the cost of merge commits in `main`.

## Recording a sync

Update `.github/upstream.json` (`lastReviewedCommit`, `lastReviewedVersion`,
`reviewedAt`) **only after the port is complete and the gates pass** — the
field means "this is the upstream this tree is", and its previous drift from
reality is what made the re-base necessary. Record user-visible changes in
`CHANGELOG.md` and add a record below.

The daily `Upstream watch` workflow reads the version from the Cargo manifest
named in `versionManifest`. Note that upstream publishes **no GitHub releases
and no tags** — both endpoints were empty even at 1.0.0, and the major bump
arrived inside an ordinary "Synced from monorepo" commit. The release/tag
comparison is not evidence that upstream has not moved.

## Re-base record: 2026-08-07 — Chutes Build 1.0.0

The tree was re-based onto `xai-org/grok-build` 1.0.0 (`afbc0fb`) rather than
continuing to cherry-pick, for the reason in "Why this changed" above.

Measured before starting, `HEAD` against `upstream/main`: 102 files existed only
here, 26 consumed `chutes-build-core`, 860 diverged only by branding, 89 were
our own non-branding fixes, and 588 diverged purely because upstream's work had
never been taken. That last number is the one that decided it.

Sequence: the Chutes layer (77 files — the other 25 "only ours" turned out to be
upstream files upstream had since restructured away), the mechanical rebrand
(5944 lines / 766 files), the ambiguous tokens decided per file (2634 / 443),
then the seams.

Four fixes had to come forward because upstream has no equivalent:
`detect_probable_secret` and the `cpk_` prefix in the redactor, the
`MediaArtifact` output type, the vendored `protoc` fallback, and the proto
dependency scan writing to a temp file instead of `/dev/stdout`.

Three defects the compiler found that review had not:

- `chutes-build-plugin` matched inside the crate name
  `xai-grok-plugin-marketplace`, renaming a workspace member.
- Bare `grok` was renamed in 25 files where it was a Rust identifier, producing
  `let chutes-build = ...`. The per-file classifier had read "the fork has zero
  occurrences" as "the fork renamed them all", when sometimes the fork had
  simply deleted the code they lived in.
- `xai-grok-pager-pty-harness` imported `process_has_exited_without_reap`
  unconditionally while the function and every call site are `#[cfg(unix)]` —
  an upstream bug visible only on Windows, worth sending back.

Followed upstream, dropping features it removed: `project_picker`, the six
bundled `SKILL.md` files, `search_remote_sync`, `upload/config_files`,
`minimum_version`, and the `coordinator_lifecycle` / `coordinator_query` split.

### Test parity, and why absolute numbers are useless here

A pristine `upstream/main` worktree was built on the same Windows machine and
measured, because upstream fails a lot of its own tests on this platform and
without that baseline the failure counts here are unreadable:

| Suite | pristine upstream 1.0.0 | this tree |
| --- | ---: | ---: |
| `xai-grok-pager --lib` | 82 failures | 82 — identical sets |
| `xai-grok-shell --lib auth::` | 24 failures | 24 — identical sets |

Zero regressions. The table stands as measured; the conclusion drawn from it does
not. "Not ours to fix" held only while nobody looked: on 2026-08-10 the pager's
sixty-six were worked through and five turned out to be defects a Windows user
would meet, not platform gaps — see "Closing the Windows failures" below. Inherited
failures are worth reading one at a time before they are filed under the platform.

The upstream failures are Windows platform gaps (path
separators, `#[cfg(unix)]` helpers, shell-alias planners) and are not ours to
fix here; three of them that broke *compilation* rather than assertions were
fixed and are worth sending back: `process_has_exited_without_reap` and
`parse_login_env_capture` are `#[cfg(unix)]` while their importers and tests are
not, and the proto dependency scan writes to `/dev/stdout`.

### What upstream's newer lints found in our code

`clippy.toml` now disallows `tokio::process::Command::spawn` — "an unenrolled
child outlives its session". The Chutes browser tool spawned headless Chrome
exactly that way, so every session that was not cleanly torn down left a browser
behind. It now enrols in the global process scope. This is the clearest argument
for the re-base: three weeks of cherry-picking never surfaced it.

### Behavioural verification, and why the gate is not enough

The gate proves the tree compiles and its tests pass. It does not prove the
product is Chutes Build. Running the built release binary found five things that
were green in CI and wrong in the product:

- `--version` printed `grok 1.0.0` — the name is a literal in `version_text`.
- `Usage:` printed `grok`, because `parse_cli` filters `argv[0]` against a known
  launcher allowlist and fell back to `"grok"` — our binary was rejected by our
  own allowlist.
- CLI help read "Run Chutes Build without the interactive UI", "Sign in to Chutes Build". 983
  occurrences of the bare product word had no rebrand rule at all.
- **The model catalog offered `grok-4.5`**, which Chutes does not serve.
  `default_models.json` had come across from upstream.
- The notification hook's test wrote `CHUTES_BUILD_MESSAGE=` labels while reading
  `$CHUTES_BUILD_MESSAGE`.

So the verification list below is not optional ceremony. At minimum, after every
sync: `--version`, `--help`, `models`, `du`, and a check that nothing was written
outside `$CHUTES_BUILD_HOME`.

Four of the bugs were in `rebrand.py` itself. The one worth remembering: a rule
and the identifier repair oscillated — one rewrote a path, the other reverted it
— and the script reported "0 files changed" while the tree sat in the wrong
state. A silent stable-but-wrong fixpoint is worse than a loud failure. Verify
idempotence by running the script twice and confirming the *second* run changes
nothing, not by trusting one clean report.

### The seam sweep, and why the file heuristic was not enough

Phase 3 identified the Chutes seams by asking *which files import
`chutes-build-core`*. That question has a blind spot: a file whose structure is
upstream's and whose values are ours. `scripts/seam_sweep.py` asks the other
question — it compares the values the previous release shipped against the
re-based tree — and found 55 identity divergences the first question could not
see. The ones that would have shipped:

| What | Was, after the re-base | Should be |
| --- | --- | --- |
| default inference endpoint | `https://api.x.ai/v1` | `https://llm.chutes.ai/v1` |
| relay / gateway WebSocket | `wss://code.grok.com/…`, `wss://grok.com/ws/gw/` | loopback sentinels |
| API-key auth.json scope | `xai::api_key` | `chutes::api_key` |
| OAuth issuer + client id | `auth.x.ai`, upstream's compiled-in app | `api.chutes.ai`; client id must be configured |
| session update/close method | `_chutes.build/session/*` | `_chutes.build/session/*` |
| npm package checked for updates | `@xai-official/grok` | `chutes-build` |

Run it after every sync, in both modes:

```
python scripts/seam_sweep.py --base <previous-release-ref>
```

`consts` mode reads like a table and is precise. `literals` mode is noisy but
sees values inside struct initialisers — which is where the endpoint table hid
while `consts` reported the file clean. Neither mode decides anything: several of
the fork's values are simply older, and several of the re-based tree's are
deliberate improvements. The output is for a person to read.

Two lessons worth keeping:

- **A wire name renamed in the handler but not the caller fails silently.**
  `` does not match after an underscore, so `_chutes.build/session/update` was
  invisible to the pass that normalised 1034 other ACP names.
- **A constant's value is not covered by the test that names the constant.**
  Every test around `XAI_API_BASE_URL_DEFAULT` asserted against the constant, so
  the endpoint could point anywhere and the suite stayed green.

### Known upstream failures on Windows

> [!IMPORTANT]
> **Superseded for the pager, 2026-08-10.** `xai-grok-pager --lib` now passes on
> Windows CI: sixty-six failures closed, five of them product defects rather than
> test bugs — see "Closing the Windows failures" below. The comparison-against-
> upstream advice no longer applies to that crate, because its baseline is zero
> and any failure is a regression. The `auth::` line below also does not match
> measurement: this tree fails 24 there on Windows, for the shell-dialect reason
> recorded in the same section.

Measured against a pristine `upstream/main` worktree on the same machine, so
these are not re-base damage:

- `xai-grok-pager --lib`: 82 failures against this tree's 79, stable across four
  consecutive runs. One of those runs had previously produced an 80th: a
  quote-bar test compares a span's style against the theme's, both read from the
  process-global `Theme::current()` that the `set_theme` tests mutate. It holds
  `pin_theme()` now, like the layout tests that had already met this.
  `xai-grok-shell --lib auth::`: 24 — all 24 of which this tree now passes.
- **`xai-grok-shell --lib` cannot be run whole on Windows**: it overflows a 1 MB
  thread stack partway through the session tests, in the debug profile only. That
  is why the gate runs `auth::` rather than the crate. `agent::config` was checked
  separately and passes 334/334 after twelve fixtures were moved onto a
  first-party host — upstream's asserted that `api.x.ai` is first-party and that a
  session token attaches to `example.com`, both of which are false under
  `endpoint_policy`. The fork had already rewritten every one of them; the re-base
  kept upstream's. Anything else in that crate's non-`auth::` tests is unmeasured
  here, and that is a gap, not a clean bill.
- `xai-grok-agent --lib`: upstream fails 5 here; this tree passes all 578. The
  five were worth fixing rather than documenting, and two of them were not test
  bugs:
  - Two skills tests compared a `String` path against a `/`-spelled suffix.
    Normalised, not skipped — the behaviour matters on Windows too.
  - `resolve_treats_home_git_repo_as_no_repo` set `HOME` and expected
    `dirs::home_dir()` to read it. On Windows that calls `known_folder_profile`,
    so no environment guard can redirect it. The comparison the guard performs is
    now a separate function, tested on every platform; the unreachable line is the
    lookup itself.
  - `refresh_skips_untrusted_source_outside_home` was pointing at a real hole:
    plugin sources are auto-trusted when under the user's home, and on Windows
    the system temp directory **is** under the home
    (`%USERPROFILE%\AppData\Local\Temp`) — so anything unpacked into temp was
    auto-trusted, skipping the explicit trust step. Temp is now excluded, and the
    five refresh-mechanics tests grant trust explicitly instead of inheriting it
    from where `tempfile` happened to allocate.
  - `test_encrypted_templates_not_stale` fails upstream too: their checked-in
    encrypted prompt bytes are stale. Ours are regenerated.
- Upstream 1.0.0 **cannot build on Windows at all**: `bin/protoc` is a DotSlash
  wrapper (`not a valid Win32 application`) and the proto build script writes to
  `/dev/stdout`. Both are fixed here, and the fix is what let the baseline above
  be measured at all.

### What a live API key found that nothing else could

Every check above passed with the credential flows listed as unverified. Then a
real `cpk_` key went in, and the first thing it proved was that **the key was not
read at all**: `XAI_API_KEY_ENV_VAR` was still `"XAI_API_KEY"` while every error
message, every doc page and the fork itself said `CHUTES_API_KEY`. A user
following the instructions would simply never authenticate. Same for the
auth-method id `xai.api_key`, which is what `preferred_method` takes in
`config.toml`. 304 occurrences across 69 files.

Three reasons the sweep had missed it, each now closed:

1. **`consts` mode compares constants by name.** The fork had renamed the
   constant *itself* (`CHUTES_API_KEY_ENV_VAR`), so the name intersection was
   empty and there was nothing to compare.
2. **The pattern was per-line.** rustfmt had wrapped
   `CLI_BASE_URL_FALLBACK`'s value onto the next line, hiding a third update
   channel that pointed at a non-existent GCS bucket — so `update --check` made a
   real request to `storage.googleapis.com` and reported `NoSuchBucket`.
   Allowing whitespace, newline included, around the `=` is the whole fix.
3. **I filtered the `literals` output by upstream hostnames.** `XAI_API_KEY`
   contains no host, so my own triage dropped it from a report that had found it.

Then 570 places told the user to run `grok login`, `grok doctor`, `grok wrap …`
— in help text, error messages, the bundled user guide and doc comments. An
instruction naming a binary that does not exist is worse than no instruction.
Now a rebrand rule, anchored on a following subcommand or flag so the internal
`grok_home` / `xai-grok-*` / `implementations/grok_build/` names stay put.

What the live run confirms:

- API-key auth works, and the model catalogue comes from
  `https://llm.chutes.ai/v1/models` — 15 Chutes models plus the `model-router`
  virtual entry.
- A real turn through the Auto router completes: ~12.9 s wall clock end to end,
  first text event at ~13.2 s in the streaming run (a cold process start, model
  resolution and routing included), then steady ~80 ms token cadence.
- Only three hosts are contacted, all Chutes-ecosystem: `api.chutes.ai`,
  `llm.chutes.ai`, `model-router-ten.vercel.app`. **No call to `x.ai` or
  `grok.com`.**
- Nothing was written outside `$CHUTES_BUILD_HOME`.

One more thing the key surfaced: a variable exported from a CI secret that does
not exist arrives as the empty string, and blank counted as *set* — so the
product announced "You are using CHUTES_API_KEY" and then failed to
authenticate, which is the least helpful pair of messages available. Blank now
counts as unset, matching the "first set, non-blank value wins" rule the
per-model `env_key` list already followed.

One thing to decide, not a defect: `model-router-ten.vercel.app` is in
`TRUSTED_HOSTS` in `chutes-build-core`, which is ours — so it is allowed to
receive an ambient Chutes credential, and `resolve_inference_base_url` routes
**session (OAuth) inference** through it. API-key auth goes straight to
`llm.chutes.ai` and never touches it. Whoever operates that Vercel deployment can
see the traffic that does go there. It was the fork's own choice; it deserves a
conscious confirmation rather than inheritance.

### Phase 4 triage: what came forward, and what did not

The 89 non-branding files we had touched, decided one at a time against the new
tree:

**Came forward** — upstream has no equivalent: `detect_probable_secret` and the
`cpk_` prefix in the redactor; the `MediaArtifact` output type; the vendored
`protoc` fallback and the proto dependency scan writing to a temp file rather
than `/dev/stdout`; the checkout-independent encrypted prompt templates
(regenerated — upstream's own checked-in bytes are stale, and the rebrand changed
the template text); voice and batch STT against the Chutes whisper endpoint; the
native Advisor; API-key login inside the TUI; `CHUTES_EXTRA_CA_BUNDLE`; the
unified secret detection; and the memory owner-only write paths.

**Correction — four of those came forward as files, not as features.** This entry
originally read as above and was wrong in the same way `.github/upstream.json` was:
it recorded an intention as an outcome. Copying a file is not porting a feature, and
a file that no `mod` declaration reaches is not even compiled, so nothing failed:

- **Batch STT** (`pcm.rs`, `stt/batch.rs`) was copied and never declared. `SttMode`,
  `batch_api_base` and the pipeline's batch branch were not copied at all, leaving
  voice with only upstream's streaming transport — pointed by the rebrand at an
  `api.chutes.ai/v1/stt` route Chutes does not serve.
- **The native Advisor** was one file, `slash/commands/advisor.rs`, undeclared. The
  built-in agent, its toolset, its prompt, the config writers and the two settings
  actions were absent.
- **API-key login inside the TUI** was one file, `slash/commands/apikey.rs`,
  undeclared, and everything it calls was missing.
- **`CHUTES_EXTRA_CA_BUNDLE`** did come forward, but through upstream's new
  `xai-grok-extra-ca` crate rather than the fork's module — which was left behind as
  dead code a reader would have taken for the live one, and is now deleted.

All four are fixed as of 1.0.0. `scripts/dead_modules.py` is the check: it lists
source files no `mod` declaration reaches, honouring `#[path]` and `include!`, and
must return nothing. Run it after any file-copying phase of a future re-base — the
"copied but unregistered" failure is silent by construction, and this re-base hit it
in four separate registries (toolsets, guide chapters, slash commands, and the
shell's reserved-name list).

**Deliberately not carried:** `trace_classifier` and the `trace_classify`
binary — 3278 lines of offline trace analysis with no consumer in the product.
The `trace` subcommand does not use them (verified: no reference from
`trace_cmd.rs` on either side), and `laziness_classifier` only mentions them in
doc comments. They remain a clean copy away in `main` if the analysis is wanted
again.

**Dropped with the code they covered**, because upstream restructured or removed
it: the Windows CI stabilisations in `ptyctl` and `xai-crash-handler` (upstream
fixed them differently), the permission/exec-risk cluster (upstream carried it
past our version), and the retry/permission work — which arrived with the re-base
itself. The toolchain pin follows upstream: the
fork had bumped to `1.94.1`, upstream 1.0.0 is on `1.94.0`, and the whole gate is
green on `1.94.0` — so there is nothing left to hold the extra point version
open.

### Credential leak, 2026-08-08: how it happened and what closed it

Two live Chutes API keys were committed to this branch, in files nobody wrote on
purpose.

The notification-hook tests interpolated an unquoted Windows path into `sh -c`.
Under Git Bash the backslashes were eaten as escapes, so `env > C:\Users\...\env.txt`
became `env > CUsersMike...env.txt` -- a redirect into the *crate root*. And
`env` prints the whole environment, so each of those files held whatever was
exported, including `CHUTES_API_KEY`. Thirty such files accumulated across the
branch; ten carried a real key. A `git add -A` swept them into seven commits.
`output/` went the same way: 42 generated media files, one of them a latency log
with a second key, tracked on this branch only -- never on `main`, never
upstream.

The remediation, in order:

1. The tests were fixed to quote the redirect target and hand the shell forward
   slashes, so no new dump can be produced.
2. `git filter-branch --index-filter` purged `output/` and every `*env.txt` from
   the 17 commits of this branch. Verified afterwards: zero commits carry a
   `cpk_`-shaped string, zero of the 10869 remaining blobs do, and the diff
   against the pre-purge tree is exactly the purged paths and nothing else.
3. `refs/original` deleted, reflogs expired, `gc --prune=now` -- the blobs are
   out of the local object store, not merely unreferenced.
4. `.gitignore` now covers `/output`, `CUsers*env.txt` and `*AppDataLocalTemp*`.
   The tests are the fix; this is the second line of defence, because a
   `git add -A` should never be able to sweep up a credential.
5. Both keys were rotated by their owner.

The branch had never been pushed, which is the only reason this was recoverable.
Two lessons that are not about git:

- **A test that dumps `env` is a test that dumps credentials.** The bug looked
  cosmetic -- stray files with silly names -- and was tidied away twice before
  anyone asked what was in them.
- **`git add -A` on a tree known to produce stray files is not a shortcut, it is
  a hazard.** Those files had already been noticed and deleted; that was treated
  as housekeeping rather than as the signal it was.

### The whole Chutes tool layer was registered nowhere

Phase 1 copied the implementations. Phase 3 reapplied the seams that *consume*
`chutes-build-core`. Neither step asked the third question — **which toolset lists
these tools** — and the answer was none of them. `generate_media`,
`list_media_models`, `describe_media_model`, `browser`, `ocr_page`,
`get_chutes_usage`: some six thousand lines that compiled, whose unit tests
passed, and which the model could never call.

Upstream's toolsets carried its own Imagine tools instead (`image_gen`,
`image_to_video`, `reference_to_video`), which talk to an xAI endpoint Chutes has
no equivalent for. Their own tier message already said so: "This legacy image tool
is unavailable. Use the native generate_media tool with a capable Chutes model."

That also explains `/imagine`. The pager hides a slash command whose
`required_tools()` are not advertised, so `/imagine` and `/imagine-video` had
quietly disappeared — they gated on tools no Chutes deployment has. Both now gate
on `generate_media`, and their injected instructions teach the catalog workflow
(`list_media_models` → `describe_media_model` → `generate_media`, composing the
payload from the cord's own `example`) because on Chutes the model is a catalog
entry rather than a fixed endpoint, and FLUX, Qwen-Image, Wan, LTX and the TTS
models share no schema. `/imagine-video` needed rewriting rather than renaming:
upstream's text opens with "there is no text-to-video tool", which is true of that
API and false here.

Restored with it: `ensure_chutes_tools` for agents whose toolset came from a
config file rather than a preset, and the lazy-discovery pair
(`take_lazy_chutes_tools` / `register_lazy_chutes_tools`) that keeps these tools
off the per-turn schema list while still registering them on the bridge for
`use_tool` — taking them off without the second half would just delete the
feature.

Two tests now assert what nothing asserted before: that the ecosystem tools are
in the default presets, and that the legacy Imagine tools are not. A tool's own
unit tests pass whether or not anything can reach it, which is exactly how this
survived a green gate.

Verified live against Chutes with a real key: `search_tool` returns
`chutes__list_media_models`, `chutes__describe_media_model`,
`chutes__generate_media`, `chutes__ocr_page` and `chutes__get_chutes_usage` with
their real descriptions, and `use_tool` invoked the catalog with
`{"kind":"image"}` — 20 requests to `api.chutes.ai`, none to any xAI host. Before
this change there was no `chutes` server in the session at all.

**Not verified: a completed generation.** The headless runs end after two tool
calls — the router model narrates its plan and stops rather than carrying the
chain through to `generate_media`. That is a headless turn behaviour rather than
evidence about the media path, and `/imagine` is a TUI command in any case, but it
means the last step of this feature is unproven here and should be exercised by
hand in the TUI.

### Auditing the documentation found a credential leak

The docs were checked against the code, not proofread. That direction matters: it
turns the documentation into a set of assertions about the product, and one of them
was true while the code was not.

`PRIVACY.md` promises the ambient Chutes credential is "never reused for a custom
endpoint" and that "a custom model-catalog endpoint uses `CHUTES_MODELS_API_KEY`".
The re-base had dropped that variable's entire auth arm, so a custom endpoint fell
into the `CHUTES_API_KEY` branch: pointing `CHUTES_BUILD_MODELS_LIST_URL` at any
host sent it your Chutes credential during startup model discovery. The promise was
the thing that caught it.

The rest was documentation being wrong about a correct product, and worth listing
because the shapes recur:

- **Vendor attribution.** The README presented this fork as SpaceXAI's, under their
  logo; `CONTRIBUTING.md` carried their "no external patches" policy;
  `SECURITY.md` routed vulnerability reports to xAI's HackerOne programme. None of
  these could be caught by any rebrand rule: "SpaceXAI" is upstream's own new name
  for itself, so no `Grok`-derived pattern touches it.
- **URLs that were assembled rather than checked.** Every install instruction was
  a 404, including the one `/docs` itself opened. Each replacement here was
  verified with a request; a plausible hostname is not evidence.
- **Controls documented as working that are compiled out.** Version pinning, the
  telemetry knobs, the coding-data row. Inheriting upstream's documentation for a
  feature this build disables produces a page that is confidently false.
- **A second registry with the same gap as the tool layer.** Two guide chapters sat
  on disk, absent from `USER_GUIDE`, so `/docs` never listed them and they were
  never extracted to `$CHUTES_BUILD_HOME/docs`. Whenever the fork adds something
  the product exposes, the question is not "is it there" but "what lists it".

### Still open

**Verified live** with a real API key (see the sections above): API-key auth, the
catalog from `llm.chutes.ai`, a complete turn through the Auto router with its
timing, the set of hosts contacted, that nothing is written outside
`$CHUTES_BUILD_HOME`, and that every Chutes tool is discoverable and the catalog
tool invocable.

**Still unverified**, and listed that way rather than assumed:

- A completed media generation. The tools are reachable and the catalog answers,
  but headless runs stop after two tool calls without reaching `generate_media`,
  and `/imagine` is a TUI command. Exercise it by hand in the TUI.
- Voice input and OCR — both need real audio and image input.
- `chutes-build login`: the browser OAuth handoff against the Chutes IdP,
  including the loopback callback and the `chutes.ai` CORS origin.
- The usage and quota display in the status bar, and `/usage`.
- The pre-stream fallback chain (`CHUTES_FALLBACK_MODELS`), which needs a model to
  be genuinely unavailable to trigger. The short-`Retry-After` wait added in
  1.0.4 needs the same: a real capacity 429 carrying the header.
- **Making `xai-grok-tools` and `xai-grok-workspace` deterministic.** Their
  failing sets move: of 53 `xai-grok-workspace` failures recorded on a local
  Windows machine, 17 pass on the Windows runner and 2 others fail, and a
  `daemonize` test failed on Linux in one run and not in the run 90 minutes
  earlier on the same commit. That is why `known_failures.py` is a local
  diagnostic rather than a CI gate — a per-test baseline needs a fixed set. This
  is the work that would make ~188 currently unwatched tests watchable, and it is
  larger than the tooling that exposed it.
- **Why `xai-grok-tools --lib` never finishes on the Windows CI runner.** It ran
  107 minutes, then 140, then hit a 40-minute timeout, against six minutes on
  Linux and about five on a local Windows machine. Every Windows step in the
  workflow runs `implementations::chutes::`, which skips
  `computer::local::terminal`, so the whole suite had never run on a runner and
  the hang had never been seen. Not reproducible from a machine where it passes,
  so the way in is a throwaway Windows job running that suite with `--nocapture`
  and `--test-threads=1` to see which test it stops on. Until then the Windows
  job gates `xai-grok-workspace` only.
- The `AuthFileStamp` memo cannot distinguish two same-length rewrites within one
  ~15ms clock tick on Windows, where there is no inode. Judged unreachable in
  practice (a rotation is minutes apart, and refresh does not use this memo) and
  left with the reasoning in place; closing it means
  `GetFileInformationByHandle`, since `file_index()` is still unstable in std.
- `xai-grok-shell --lib` as a whole: it overflows a 1 MB thread stack on Windows in
  debug, so that crate's non-`auth::` tests are unmeasured here. `agent::config`
  and `remote::client` were checked per-module and pass.

### The pending port: 3 commits, and which 91 files are safe

Measured 2026-08-11 against `upstream/main` at `b13fa526`, from the recorded
baseline `afbc0fb7`. Three commits, 238 files, +17887/−6138.

Do not measure this with `git diff HEAD upstream/main`. That reports 1424 files
and 40k deletions, nearly all of which are *our* re-base and rebranding read
backwards — the fork is not a descendant of upstream, so the diff is symmetric
and tells you nothing about what arrived. Diff from the baseline commit instead.

What the three commits contain:

| Commit | Size | Theme |
| --- | --- | --- |
| `8a14c91d` | 157 files, +11148/−4949 | session replay (`session/storage/replay.rs` + tests, 1627 lines, new), `app/session_load_barrier.rs` (new), `scrollback/state/layout.rs` (new), `.envrc`/direnv support in workspace |
| `75e73f3d` | 10 files, +273/−789 | `agent/app.rs` loses 740 lines to a refactor; sandbox `hook_write_deny` and `child_net`; textarea |
| `b13fa526` | 102 files, +6536/−470 | session rename as a user-facing feature (`slash/commands/rename.rs`, `session_admin.rs`, `agent/server.rs`) |

**The split that makes this tractable.** Intersect what upstream changed with
what we changed, both measured from the same baseline:

- **147 files overlap.** These need reconciling by hand, one at a time.
- **91 files are upstream-only** — we never touched them, so there is nothing of
  ours to lose. 17 are new files; 74 already exist here unmodified.

The overlap list is the argument against merging, in concrete form: it contains
`xai-grok-agent/templates/prompt.md` and `prompt/prompt_encrypted.rs`. A merge
would take upstream's system prompt again and silently re-introduce exactly the
regression 1.0.3 fixed — the agent introducing itself as xAI's, and the
`<official_chutes_sources>` block with its "never put credentials in a search
query" instruction deleted a second time.

**Of the 91, only 16 carry branding tokens** in upstream's new version (checked
with `grep -E 'GROK_|grok-build|GrokBuild|Grok Build|grok\.com|x\.ai|~/\.grok'`
against `upstream/main:<path>`, not against ours):

```
app/exit_timeout.rs                     agent/handlers/models.rs
app/session_load_barrier.rs             agent/mvp_agent/tests/session_rename_tests.rs
memory_trace_signal_topology_tests.rs   session/acp_session_impl/spawn_runtime_containment_tests.rs
pty_e2e/scroll_anchor_holds_parked_…    session/goal_planner.rs
pty-harness/tests/exit_timeout.rs       session/goal_strategist.rs
session/goal_summarizer.rs              session/storage/relocation/mod.rs
session/storage/replay.rs               session/storage/replay_tests.rs
tests/test_mcp_doctor_isolation.rs      workspace/src/envrc.rs
```

The other 75 are token-clean and can be taken verbatim, subject only to API
drift at compile time. Run `scripts/rebrand.py` over the 16 — it exists for
this and is re-runnable.

Order the work by area, not by commit, and land each area with its own tests
green: it is the only way to tell which area broke something.

#### Let the machine sort the delta first

`python scripts/port_assist.py --base afbc0fb7` splits the 238 files five ways
and only the last needs a person:

```
done          30  (12%)  already ported — we hold upstream's version
new           13  ( 5%)  not in our tree
clean         62  (26%)  untouched by us since the baseline
mechanical    52  (21%)  our divergence is exactly the rebrand
manual        81  (34%)  genuine divergence — read it
```

`mechanical` is the bucket worth understanding: for those files
`rebrand(upstream@base)` reproduces our current version character for
character, which proves our only divergence was the rebrand and that
re-applying it to upstream's *new* version reproduces the work exactly.

`--apply` writes the three take-able buckets and leaves `manual` alone. It is
where the work starts, not where it ends — every area ported so far reached two
to four crates, so compile, run the suite and check `known_failures.py` before
believing any of it.

A useful sanity check on the tool: `--path crates/codegen/xai-grok-agent`
reports 100% manual. That is the crate holding `templates/prompt.md`, and
taking it automatically is exactly what told every model it was released by xAI.

#### Ported so far

Four areas, 28 files, on `port/upstream-b13fa526`. Each landed as its own commit
with its own tests green, measured against a pre-port baseline.

| Area | Files | What it brings |
| --- | ---: | --- |
| `xai-tty-utils` | 5 | Runtime worker sizing against `pids.max` / `RLIMIT_NPROC`, plus 329 lines of tests it had none of, including EAGAIN |
| Skill-path suggestion | 6 | A failed `SKILL.md` read answers with where the skill is registered |
| Workspace RPC | 17 | Every RPC declares read or write (`ACTIVITY`); proxy client reports the server binary version |
| Sandbox write-deny | 3 | Compiled only where it enforces — Linux, plus the test arm |

Sixty-six of the ninety-one safe files remain, and all hundred and forty-seven
overlapping ones.

#### What those areas taught

Four things generalise to the rest.

**An area is bigger than the module you meant to take.** The suggestion looked
like one new module. Alone it fails clippy's `dead_code`, because its only
consumer is the `read_file` tool; `read_file` then needs
`AsyncFileSystem::file_exists`, new in `computer/types.rs`, with its local and
mock implementations. Six files for one feature. **`dead_modules.py` cannot see
this** — the module *is* reachable through `mod`, so it reports nothing. The
lint is the detector, which means an area is not finished until
`cargo clippy -p <crate> --all-targets -- -D warnings` passes, not merely until
it compiles and its own tests are green.

**Measure the baseline failure set before attributing anything.** This tree has
pre-existing Windows failures — 69 in `xai-grok-tools` alone — so a post-port
count is meaningless on its own. Stash, run, save the sorted failure names, run
again after, and `comm` the two lists. "73 failures" told us nothing; "these
exact four are new" told us everything.

**Upstream's fixtures assume POSIX, and their CI never says otherwise.** Eleven
tests across the two areas failed here and pass upstream, all for the same
reason: a leading slash on Windows is drive-*relative*, so
`Path::new("/repo/x").is_absolute()` is false and any code guarding on
`is_absolute()` skips the whole fixture set. The production guards were right
every time. Give the fixtures a dialect — a small `abs()` helper that prefixes a
drive on Windows — rather than touching the code they cover. Watch for the
second form too: a path built with `join` carries host separators, so an
expected string must be composed the way the implementation composes it, not
written with forward slashes.

**Do not run `scripts/rebrand.py --apply` over the whole tree and trust it.** It
used to rewrite the six sentences that name upstream on purpose — the README
attribution, SECURITY's upstream-scope paragraph, the comments explaining whose
wordmark 1.0.0 shipped, and the fork link in the getting-started guide, which it
pointed at a repository that does not exist. Those are pinned in `UPSTREAM_PROSE`
now and the script fails loudly if one is reworded. Still read `git status` after
every run: a file outside the area in hand is the signal that something was
matched that should not have been.

### The Windows CI job crashes, and it is not a test failure

> [!NOTE]
> **Closed.** The crash was the doctor's voice probe: `collect_report` enumerated
> the audio device through WASAPI, and doing that twice in one process takes the
> binary down with `0xc0000005`. The report now skips the probe entirely when it is
> not asked for, rather than suppressing its finding — the first attempt did the
> latter, the probe still ran, and CI returned the identical crash. With the log no
> longer truncated, the run finally produced a complete failure list, which is what
> "Closing the Windows failures" below works from. The `RUST_TEST_THREADS=1`
> localisation described here was never needed.

`Rust quality (Windows)` is red on `main` since 1.0.0 landed. The pager test binary
**exits with `0xc0000005`** — an access violation — after roughly 3868 of ~8200
tests, so `cargo test` reports no failure list at all: the process dies mid-run.
`Rust quality (Windows)` was green on the 0.4.x tree, so this arrives with upstream
1.0.0's code; it is not something the re-base introduced into it.

**The `git2` 0.20.4 bump is excluded, by experiment rather than by argument.** Every
Windows run between the landing commit and that bump was cancelled by a later push,
so the timeline alone could not separate them — and the crash sits at the boundary of
`git_info`, which is the module that calls libgit2, with the bump carrying five
libgit2 patch releases. Dispatching CI on the landing commit (`2ece18b`, `git2
0.20.2`) reproduced it exactly: same `0xc0000005`, same step, 3868 tests reported
against 3869 and 3871 on the bumped runs. Nothing to look for in libgit2.

Ruled out, so nobody repeats the work:

- **The escape sequences immediately before the crash are innocuous.** The log shows
  Kitty graphics deletes (`_Ga=d,d=i,…`) and OSC 12 cursor colours. Both are
  pure string construction plus a locked-stderr write — `terminal::image::clear_kitty_image`
  and `theme::apply_cursor_color`. They are interleaved output from a parallel run,
  not the fault.
- **Not the PTY tests.** Every test in `pty_wrap.rs` is `#[cfg(unix)]`.
- **Not `SetConsoleCtrlHandler`.** That FFI lives in the production
  `screen_mode_relaunch` path, not in a test.

To localise it, dispatch the workflow with the Windows job running single-threaded
(`RUST_TEST_THREADS=1`). With one thread the harness prints each test's name *before*
running it, so the last name in the log is the one that died — which a parallel run
cannot tell you. Do that before guessing at the 56 `unsafe` blocks in the pager.

Three `doctor_cmd` tests also fail on Windows — `non_tty_without_yes_fails_safely_before_write`,
`fix_preview_contains_exact_change_and_caveats`, `decline_is_success_and_does_not_write`.
They fail on this machine too, and failed before the re-base, so they are a separate,
smaller problem from the crash.

The seams in `agent/{config,init,models}.rs` and `extensions/billing.rs` are
applied, and the memory store now writes every path through
`write_secure_file` / `ensure_owner_only_permissions` (three of the four paths
still created files with the process umask; the fourth already did not, and only
the privacy filter had been wired up). `memory_files_are_owner_only` covers all
of them on Unix.

`main`'s branch protection still forbids the merge commits this model needs; see
the section above. That is a decision, not a task.

### Closing the Windows failures, 2026-08-10

Sixty-six, from CI's own list — available complete for the first time once the
crash above stopped truncating the log. Five were product defects, and they are in
the 1.0.0 changelog: `doctor fix` unable to write anything, file paths in agent
prose never clickable, Tab completion in the extensions modal destroying the path,
the memory header painting `\MEMORY.md`, and a fixture named `nul` that on Windows
is the null device. The rest were fixtures describing a platform they were not
running on.

Three things are worth carrying forward more than the individual fixes.

**A red step hides everything below it.** The Windows job's second step — the
authentication and tools tests — had *never executed*, because the pager step
above it always failed first. The Linux job had not reached its end in twenty
runs: first a formatting failure, then one flaky test, and behind them four
unexecuted steps including clippy over the Chutes-owned packages. Every step
cleared reveals the next, and the count of "remaining failures" is only ever a
lower bound until a job runs green end to end.

The last thing hiding down there was not a test at all: **a job that starts
passing does more work than the one that used to fail, and can outgrow the time
limit written for the broken version of itself.** The Linux job's 60 minutes had
been ample while it died at the formatting check; the first run that reached the
end spent 59.7 minutes on steps that all passed and was killed during the cache
*save*. That failure mode feeds itself — no cache saved means the next run starts
cold, takes longer, and dies in the same place — so the limit is now 90.

Measured twice after the raise: 65 minutes, then 67. Which corrects the guess
made when raising it — that a warm cache would leave the real cost well under the
limit. It does not: about 45 of those minutes are test execution and 11 are
clippy, so compilation is not what fills the hour, and the headroom is roughly a
quarter rather than a half.

Final state, 2026-08-10: all five jobs green, which is the first complete run this
repository has had. `xai-grok-pager --lib` 8274 passing on Windows against 83
failures at the start, `xai-grok-shell --lib auth::` 374 against 24 in a step that
had never run.

**Reproduce the runner rather than trusting the local number.** Local runs on the
development machine disagreed with CI in both directions, and each disagreement
had a cause worth knowing: the runner's `%TEMP%` is an 8.3 short path
(`C:\Users\RUNNER~1\…`), which a guard that rejects `~` refuses; `/tmp` is
drive-relative off Unix and resolves only because `C:\tmp` happens to exist here,
while the runner works on `D:`; and a terminal is attached here and not there, so
capability-driven rendering and key handling differ. Pointing `%TEMP%` at a
short path turned a CI-only failure into one that could be iterated on locally in
seconds.

**A widened deadline is usually a misdiagnosis.** The history-delivery test was
"fixed" once by taking its budget from one second to ten. It kept failing, because
the condition was `tick() && result_count() == 2`: `tick()` reports that the screen
needs repainting, not that the daemon answered, and `activate` takes an eager
snapshot that moves the generation counter — so when the daemon is *fast*, the
results are already there, no later poll reports a change, and the conjunction can
never hold however long it waits. It fails when the machine is quick, which no
amount of clock could fix.

### The auth suite, and the step below the step

Clearing the pager let the Windows job reach its second step for the first time —
`xai-grok-shell --lib auth::`, which failed 24. One cause: a provider command goes
through the platform shell, and off Unix `util::subprocess::shell_c` picks `cmd /C`
deliberately, because the contract is "exit 0 means success" and PowerShell's
`-Command` does not propagate a child's exit code. The fixtures were POSIX
one-liners — `printf`, `sleep 20; printf never`, `$(wc -l < …)`, `${VAR:-0}`,
`if [ … ]; then … fi` — so they minted nothing there.

Writing each fixture twice, once per dialect, would have encoded the problem
instead of removing it: `cmd` has no `sleep`, no `printf`, no default-valued
expansion, and quoting JSON through it is its own nightmare. The suite now drives
`crates/codegen/xai-grok-shell/src/bin/auth-provider-fixture.rs`, invoked with
`args`, which takes the direct-exec branch where no shell interprets anything.

Three constraints that shaped it, worth knowing before touching it:

- **The helper has to be built first.** `CARGO_BIN_EXE_*` is set for integration
  tests and benches, not for a lib's unit tests, and `cargo test --lib` does not
  build a crate's binaries. Both CI steps and the local gate build it; the test
  panics with the exact command if it is missing.
- **The command-only paths cannot be quoted.** `run_external_refresh` and
  `run_external_auth_provider` take a command and no args, so they still go through
  a shell — and `cmd /C` strips the first and last quote of the whole string, so a
  quoted program with quoted arguments comes apart in its hands. The helper is
  named bare there, with an assertion that nothing in the line contains a space.
- **A fixture must not abort.** `[profile.dev]` sets `panic = "abort"`, so the
  stderr-flood mode writing through `eprint!` — which panics on a failed write —
  surfaced as exit `0xc0000409`, indistinguishable from the deadlock that case
  exists to detect.

The two lock tests were a different fault: they read the lock file through a second
handle while holding it, which Windows refuses with `ERROR_LOCK_VIOLATION`, since
`fs2` locks a byte range there. They read through the holding handle now, which is
what `holder_state` does in the product. Note what that implies for production:
under real contention the PID-liveness read fails on Windows and the code falls
into the `holder info unreadable` branch it already has, so staleness is decided by
mtime. It degrades safely — it waits the full threshold before breaking a lock —
but the liveness probe does not run there.

374 pass, 0 fail.

## Review record: 2026-08-07

Upstream moved for the first time since the fork's last two passes, and it
reached **1.0.0**: six `Synced from monorepo` commits carried `0.2.117` to
`1.0.0` (`a4221165` → `afbc0fb`, 2026-08-03 to 2026-08-07), touching 748 files
with roughly a hundred and ten changes.

A note for future passes, because it misled this one: upstream still publishes
**no GitHub releases and no tags** (both endpoints return empty at `1.0.0`), and
the major-version bump has no commit of its own — it landed inside an ordinary
`Synced from monorepo` commit alongside seven unrelated changes, one commit
after "Drop the Beta label from the product". The version exists only in the
lockstepped Cargo manifests. `.github/upstream.json`'s `versionManifest` field
and the `Upstream watch` workflow read exactly that file, which is the correct
signal; the release/tag comparison is not, and should not be read as evidence
that upstream has not moved.

Measured against the reviewed baseline at the time of the port work
(`393430ee`, `0.2.121`), 81 of the touched files were still byte-identical in
this fork and could be taken as-is; 491 had diverged and needed hand-merging;
57 did not exist here at all. The `1.0.0` commit adds 77 more files and does
not overlap the areas ported below.

Ported — security:

- Lexical path normalization before the permission glob match, with the
  session cwd threaded through the manager and the shell-file gate. This is the
  fix with real bite: `Read(src/**)` was escapable as `src/../../etc/passwd`
  because `**` consumes `..`. Seven regression tests cover traversal escapes,
  deny reach, the `*` catch-all, and the no-cwd path; their absolute fixtures
  are built from a platform root, since `/etc/passwd` is not absolute on
  Windows and would otherwise be cwd-joined and stop testing the escape.
- `NotebookEdit` / `NotebookRead` rejected as rule prefixes instead of aliasing
  onto `Edit` / `Read`.
- Vendor-compat MCP kill switch actually enforced against client-forwarded
  servers, including the on-disk attribution loader that bypasses the
  import-marker cutoff, applied at all three ingress points.
- Char-safe bearer fragment. Upstream extracted it into a new
  `xai-grok-auth::bearer_fragment`; this fork had the same byte-slicing bug in
  its own `token_suffix`, which now delegates to that one definition.
- Sandbox deny-glob rework: the 200k visited-entry cap that refused startup on
  ordinary large workspaces, symlink-target masking, and fail-closed walk
  errors. `deny/glob.rs` was identical to the baseline, so it was taken whole;
  only the caller's `Option` → `Result` seam and the caps struct were adapted.

Ported — inference resilience, the highest-value area for this fork because
Chutes is served through the Cloudflare edge:

- `RetryPolicy::edge_client` (429 + any 5xx minus origin-TLS 525/526), and
  `SamplingError::is_retryable` rebuilt on it. The previous hard-coded list
  made 521–524, 529 and 530 fatal, so a transient edge failure ended the turn.
- `Retry-After` clamped to a new `MAX_RETRY_BACKOFF` of 30s and jittered on the
  generic path, leaving the 429 path to wait the full value under its own
  attempt cap.
- `x-should-retry` carried through `SamplingErrorInfo`, so the header survives
  stream collection.
- Clean non-200 banners. This one predates the reviewed baseline: upstream's
  `provider_error.rs` and `user_facing_api_error_message` were never taken at
  the fork point, so `parse_error_bytes` still fell back to the raw body —
  meaning a Cloudflare HTML page went straight to the terminal. The module was
  added and all nine client call sites now pass the status.

Not portable: `gate_preflight.rs`. The module itself is small, but it is built
on upstream's `GateDecision` provenance rework (`AskRuleMatch` vs
`AskFailClosed`) and the `manager.rs` → `manager/` split, neither of which this
fork has. The security value it carries — the cwd-aware evaluation — was taken
directly instead, via `evaluate_with_cwd` at the manager and shell-file gates.

Excluded by policy, as in previous passes: the three telemetry additions (Auto
decision, `shortcut_used`, model-side skill reads) and `agent/otel_gate.rs`;
the `/feedback` card changes; the "run chutes-build update before re-authenticating"
copy; the cloud sandbox provisioning types and `remote/skills_client.rs`.
Excluded as upstream-specific: dropping the Beta label, the `chutesday` /
`chutesnight` themes, the Finance `ToolUsageCard` variant, and a deprecation-doc
path fix for a file this fork does not have. Not applicable: the toolchain bump
to 1.93.0 (this fork is on 1.94.1) and the workflow-subagent cap, which needs
the upstream-only `xai-workflow` crate. Deliberately not taken: removing the
project-directory picker, a feature this fork keeps.

Ported — presentation and prompt:

- Narrow markdown tables reflow inside their cells. The hard split walks
  grapheme boundaries (CJK, VS16 emoji, ZWJ clusters stay intact) and the span
  projection uses a monotonic cursor, so a plain run repeating an earlier
  substring cannot inherit a link's style or hyperlink range. All four touched
  files were identical to the baseline and taken whole; the crate gains
  `unicode-segmentation`, already a workspace dependency.
- Plan-viewer scrollbar: grab zone widened to the border column, striped thumb
  fixed on Terminal.app.
- Session recaps follow the language of the user's own chat messages instead of
  always English. This fork's copy of the prompt was also older than the
  baseline and had lost the explicit "do not call tools" guard, which came back
  with it.

Arrived with `1.0.0` itself and not yet ported: guarding the in-process git
status/diff from client spam, memory traces bundled into the session trace
export (local-only here, so no privacy conflict), a tabbed usage/session-info
modal behind `/usage`, `/session-info` and `/context`, `startupHints` on
session request metadata with the headless MCP connecting reminder fixed, and a
plugin CTA debounce. Naming the Windows download `Chutes Build Setup.exe` is upstream
distribution and excluded.

Looked at and deliberately deferred rather than rushed: skipping nested
checkouts in file watching. The visible half is a small new `checkout.rs`, but
the fix that matters lands in `watcher.rs`, which upstream grew by 206 lines
and which has diverged here (Sapling support sits behind
`CHUTES_BUILD_FSNOTIFY_SAPLING`). It needs its own pass, not a hurried merge
into a live filesystem watcher.

Still open at the end of this pass, in rough value order: session-runtime work
(streamed session fork, bounded post-kill reaps, the shared search-index lock,
leader-disconnect eviction, the restored-child session registry behind
`--resume`), tool and agent-loop fixes (MCP images extracted before truncation,
read-only tool metadata, colliding skills, the goal evaluator, the stop gate),
the TUI batch (scrollbar, CJK selection, markdown palette and table reflow,
tmux/SSH theme detection, teardown resets, prompt-queue ordering), and the
user-facing additions that would justify a minor release (`du`, ACP
`session/resume` / `session/close`, conversation-only `/rewind`, the permission
pattern editor, the dashboard batch).

The baseline in `.github/upstream.json` is therefore **not** advanced by this
pass: the procedure above only allows it once the selected ports are complete
and the gates pass, and the selection is still open.

## Review record: 2026-08-02

Upstream `main` was still `a4221165824e5b1f5c4c10b7459f65e78dd6448d` (`0.2.117`),
unchanged since the previous review, and upstream still publishes no releases or
tags. No new upstream work existed to review, so this pass reopened the areas the
2026-08-01 review deferred.

Ported:

- MCP config persistence. The four MCP writers parsed `config.toml` with a
  fallback to an empty table and then rewrote the whole file, so an unparseable
  config was replaced by only the keys being saved; none took the config write
  lock, and all staged through a fixed temp filename. Now one helper refuses
  unparseable input, holds the user-config lock across the read-modify-write,
  writes atomically, and skips no-op writes.
- Extra TLS roots via `CHUTES_EXTRA_CA_BUNDLE`, for proxy-terminated TLS.
  Reworked rather than copied: upstream adds a crate that depends on `rustls`
  directly, which would mean editing the generated workspace root, so this uses
  reqwest's own PEM bundle parsing inside `xai-grok-http`.
- Cancelling an in-flight `/compact`, including the `is_compact_running` /
  `cancel_compact_command` state predicates it needs.

Already present, nothing to port: background subagent cancellation (identical,
tests included), terminal resize (`ptyctl` differs only by a comment and an
upstream-only lint allow), and session cold-start (voice cold start, already
implemented).

Not portable: the `auth_retry` module behind the ACP task-lifecycle changes is
bound to upstream's `AuthManager`/`SentCredential` flow. Chutes Build
authenticates differently, so adopting it means reworking authentication, not
porting a module.

Deferred with a known reason: `streaming-messages-json` (NDJSON in the Messages
API wire format). Upstream implements it as a reducer split across 19 files;
this fork still has a monolithic `headless.rs` that branches on the output
format in twelve separate places. The port is worth doing but needs the reducer
refactor first, not a thirteenth branch.

The baseline in `.github/upstream.json` is unchanged because upstream itself has
not moved.

## Review record: 2026-08-01

Reviewed upstream `main` through `a4221165824e5b1f5c4c10b7459f65e78dd6448d`
(`0.2.117`). Upstream still publishes no GitHub releases or tags, so the
versioned changelogs and `main` commits were the authoritative review inputs.

Ported selectively:

- `0.2.115`: deliver the repeated-tool-call stationarity reminder only after
  the preceding tool results are committed, preventing duplicate tool results
  from corrupting chat history;
- `0.2.116`: accept `/undo` as an alias for `/rewind`; and
- `0.2.117`: require revision notes before Enter requests plan changes, with an
  explicit `a` shortcut for approval.

Already covered by Chutes-specific code: Windows external-auth shell handling
uses the platform-aware command builder and retains its dedicated tests.

Deferred because they are coupled to newer upstream runtime/configuration
surfaces and need a separate Chutes compatibility review: MCP enable/disable
persistence, streaming JSON events, additional CA-bundle handling, background
subagent cancellation, ACP task lifecycle changes, terminal resize behavior,
and session cold-start work. No upstream identity, billing, telemetry, or
enterprise-only behavior was imported.

## Review record: 2026-08-21 — full sync to `d71f6e0c`

Upstream advanced from the `afbc0fb7` (1.0.0) baseline to `d71f6e0c`
(`Synced from monorepo`, version `1.0.5` in the pager-bin manifest), in ten
commits. The full delta — 968 files — was ported by hand, one area per commit,
on `port/upstream-d71f6e0c`.

What upstream carried, in broad strokes: the session events log split out of
`xai-file-utils`, the active-sessions and foreign-sessions registries split out
of the shell and workspace, SQLite FTS search over local sessions, an
in-guest loopback diagnostics server, the workspace-server daemon split, the
subagent bundle cache, the compaction-transcript renderer, and the
fuzzy-file-search walker — eleven new crates registered in the root manifest.

### Ported

- **Runtime infrastructure** — fast-worktree, fsnotify, file-utils, tty-utils,
  chat-state, agent-lifecycle, config, config-types, sampler, sampling-types,
  hooks, hooks-plugins-types, shared, shell-base, test-support, telemetry,
  memory, mcp, tools, and the tool/computer-hub protocol crates.
- **`xai-grok-agent`** — the prompt structure and `AgentDefinition` moved to the
  upstream shape, with a new runtime-only `builtin_name` keying the
  browser-verification gate. `templates/prompt.md`, `prompt_encrypted.rs` and
  `skills.rs` stayed byte-identical to the fork: the Chutes system prompt,
  subagent prompt and persona text were not taken from upstream.
- **`xai-grok-shell`** — the session lifecycle, ACP, and config. The media-gen
  parallel-call caps the fork added converge with an independent upstream
  `media_gen_limits`; our version matches theirs exactly after rebrand.
- **`xai-grok-workspace`** and **`xai-grok-pager`** — workspace/RPC, TUI and
  launcher, with their Chutes seams preserved.

### Windows-only defects found while verifying (upstream CI runs Linux/macOS)

Four ported paths failed only on Windows, none caught by the build or the
narrow test filters — `models` on the built binary and the leader/worktree
suites did:

- `WorktreeDb::get` decided "is this a path" with `contains('/')`, so a
  backslash path on Windows fell into the id/label branch and the DB lookup
  never ran.
- `get_worktree_info` compared a git2 forward-slash `commondir` against a
  native backslash path; the display now re-assembles native components.
- The model-list client sent `x.ai/models/list` while the handler registered
  `chutes.build/models/list`, so `models` failed at runtime (`unknown ACP
  extension method`). The ambiguous `x.ai/` token is not rewritten by
  `rebrand.py`, so 173 test fixtures were realigned to `chutes.build/` by hand.
- A trailing-dot fixture name (`notes.txt.`) cannot exist on Windows and was
  being normalised into a valid capability, so it is now `#[cfg(not(windows))]`.

### What stayed manual

`xai-grok-agent` remains 100 % manual by policy: the three identity files above
were never auto-taken, and their divergence was confirmed byte-identical to
the fork's before the agent commit.

### Verification

`cargo check --workspace --all-targets`, `cargo fmt --check`, clippy
`-D warnings` on the gate crates, `dead_modules.py` (all reachable),
`seam_sweep.py` against both the baseline and `v1.1.0`, and the release binary
(`--version`, `--help` branding sweep, `models`, `du`) all green. Gitleaks scans
the worktree and the full history clean.
