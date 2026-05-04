# Self-hosted CI for integration tests

Scribe's end-to-end integration test
(`crates/scribe/tests/transcribe.rs::transcribe_jfk_end_to_end`) shells out
to `ffmpeg` and `whisper-cli` and needs a real Whisper model file on disk.
On the GitHub-hosted `ubuntu-latest` runners that handle PR validation in
[`ci.yml`](../.github/workflows/ci.yml), none of those dependencies are
present, so the test self-skips with a clean pass. That's a deliberate
design choice (see the test's module doc), but it means CI on PRs never
actually exercises the pipeline.

This document describes the self-hosted runner pattern that fixes that gap
post-merge, defined by [`integration.yml`](../.github/workflows/integration.yml).

## Why self-hosted

`whisper.cpp` with CUDA support is non-trivial to build, the GB10 GPU on
the DGX Spark is the canonical raibid-labs target, and the model files are
hundreds of megabytes. Reinstalling all of that on every CI run on a
generic GitHub runner is impractical — and even if it were practical, we
wouldn't get GPU acceleration. A long-lived self-hosted runner on a host
that already has the toolchain (the Spark) is the natural fit.

## Security model — read this first

`raibid-labs/scribe` is a **public** repository. GitHub's
[security hardening guide for self-hosted runners](https://docs.github.com/en/actions/security-guides/security-hardening-for-github-actions#hardening-for-self-hosted-runners)
is unambiguous: do not run a self-hosted runner against `pull_request`
events from a public repo, because forked PRs can run attacker-controlled
code on the runner host.

The integration workflow enforces this in two ways:

1. **Trigger surface.** It runs only on `push` to `main` (post-merge — the
   code has already been reviewed) and `workflow_dispatch` (manual, gated by
   repo write access). There is no `pull_request` trigger and one must not
   be added.
2. **Checkout discipline.** `actions/checkout@v4` runs without a
   caller-controlled `ref:`, with `persist-credentials: false`. No
   `${{ github.event.inputs.* }}` value is interpolated into a shell string.
   `permissions:` is `contents: read` only.

PR-time validation (formatting, clippy, the unit + skip-mode integration
test) continues to run on the GitHub-hosted runners defined in
[`ci.yml`](../.github/workflows/ci.yml). That side of the pipeline is
unchanged; nothing in this document affects it.

If you need to validate a branch on the self-hosted runner before merging,
trigger `workflow_dispatch` against your branch from the Actions tab. Doing
that requires repo write access, so it's not an attacker vector.

## Runner host requirements

The runner host must have:

- **CPU/OS:** Linux. The Spark is aarch64; the workflow currently labels
  `ARM64`. If you add an x86_64 runner later, give it `X64` and add a matrix
  entry to the workflow.
- **`ffmpeg` and `ffprobe`** on the runner user's `PATH`. Debian/Ubuntu:
  `sudo apt install ffmpeg`.
- **`whisper-cli`** — the canonical raibid-labs build is in
  `~/raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli`.
  Either symlink that onto `PATH` or set `SCRIBE_WHISPER_BIN` for the
  runner service (see the systemd unit example below).
- **At least one Whisper model `.bin`** under
  `~/raibid-labs/voice-stuff/models/` (or pointed to via
  `SCRIBE_MODEL_PATH`). `large-v3` is the typical choice; `base.en` is
  sufficient to make the test pass and is much smaller.
- **A working Rust stable toolchain.** The workflow installs Rust via
  `dtolnay/rust-toolchain@stable`, which uses `rustup`. Make sure the
  runner user has `rustup` available, or pre-install Rust system-wide.
- **Network egress** to `github.com`, `crates.io`, and
  `static.crates.io` for the runner protocol and `cargo fetch`. No inbound
  ports need to be open — the runner connects out.

## Required runner labels

The workflow targets:

```
runs-on: [self-hosted, Linux, ARM64, scribe-integration]
```

When you register the runner, advertise these labels:

- `self-hosted` — applied automatically by GitHub.
- `Linux` — applied automatically.
- `ARM64` — applied automatically on aarch64 hosts.
- `scribe-integration` — **custom**, must be added explicitly during
  `config.sh`. This is the label that gates which hosts the workflow is
  willing to land on. Without it, an unrelated self-hosted runner attached
  to the org/repo would not match this workflow.

## Registering a runner

There are two equivalent paths. Either uses the same registration token,
which is short-lived (one hour) and obtained from
`https://github.com/raibid-labs/scribe/settings/actions/runners/new`.

### Path A: GitHub UI walkthrough

1. Browse to
   <https://github.com/raibid-labs/scribe/settings/actions/runners/new>.
   You need repo admin to view that page.
2. Pick **Linux / ARM64**. GitHub generates a `curl` snippet to download
   the runner tarball plus a `./config.sh` invocation that includes the
   registration token. Copy it.
3. SSH into the runner host as the dedicated runner user (see hardening
   below). `mkdir -p ~/actions-runner && cd ~/actions-runner`.
4. Paste and run the GitHub-provided snippet — it downloads the runner,
   verifies the SHA-256, and runs `./config.sh`.
5. When `config.sh` prompts for labels, append `scribe-integration`:

   ```text
   This runner will have the following labels: 'self-hosted', 'Linux', 'ARM64'
   Enter any additional labels (ex. label-1,label-2): scribe-integration
   ```

6. When it prompts for a work folder, accept the default `_work` (the
   recommendations below assume an isolated path under the dedicated user's
   home directory anyway).
7. Either run `./run.sh` interactively to confirm it connects, or skip
   straight to the systemd setup below.

### Path B: CLI registration token via `gh api`

If you'd rather not click through the UI, you can mint a registration
token from the command line:

```sh
unset GITHUB_TOKEN
TOKEN=$(gh api -X POST /repos/raibid-labs/scribe/actions/runners/registration-token --jq .token)
```

Then on the runner host, after extracting the runner tarball:

```sh
./config.sh \
  --url https://github.com/raibid-labs/scribe \
  --token "$TOKEN" \
  --labels scribe-integration \
  --unattended \
  --replace
```

`--unattended` skips all prompts. `--replace` lets you re-run config
against the same name without erroring (useful when reinstalling).

The token expires in one hour. If you need to register multiple runners,
mint a new token per host.

## Recommended hardening

The runner executes code on every push to `main`. Treat the runner host
the way you'd treat a build-and-deploy box.

### Dedicated unprivileged user

Create a `gh-runner` user with no sudo rights:

```sh
sudo useradd --create-home --shell /bin/bash gh-runner
```

Install the runner under `/home/gh-runner/actions-runner/`. Do not run
the runner as root, and do not give the runner user `sudo` for any
command — anything the integration test needs (ffmpeg, whisper-cli, the
model files) should be readable by the runner user without elevation.

### Isolated work directory

Configure `config.sh --work /home/gh-runner/_work` (or accept the default,
which is `actions-runner/_work` under the runner home). The work directory
is wiped between jobs but must remain inside the runner user's home so it
inherits the user's permission boundary. Do not point it at a shared
location.

### systemd unit with auto-restart

The runner ships a `svc.sh` helper that installs a systemd unit. Run it
as the runner user (it'll `sudo` only the install step):

```sh
cd /home/gh-runner/actions-runner
sudo ./svc.sh install gh-runner
sudo ./svc.sh start
```

If you need to inject `SCRIBE_WHISPER_BIN` or `SCRIBE_MODEL_PATH` for the
test, edit the resulting unit (`/etc/systemd/system/actions.runner.<repo>.<name>.service`)
and add an `EnvironmentFile=` pointing at a root-owned, runner-readable
file (e.g. `/etc/scribe-runner.env`). Reload with
`sudo systemctl daemon-reload && sudo systemctl restart actions.runner.<repo>.<name>`.

### Firewall posture

The runner only needs **outbound** HTTPS. There are no inbound ports to
open. On a default UFW configuration this is a no-op:

```sh
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw enable
```

If the runner host also serves something else, scope the rules
accordingly — the runner itself doesn't accept connections.

### Don't share the host with secrets

The runner host should not also store production secrets, SSH keys for
unrelated services, or other long-lived credentials. The integration
test is fully self-contained (it transcribes a public-domain JFK clip);
nothing on the host beyond ffmpeg + whisper-cli + a model needs to be
sensitive.

### Keep the runner updated

Self-hosted runners auto-update by default. Don't disable that. Subscribe
to <https://github.com/actions/runner/releases> if you want a heads-up on
breaking changes.

## Removing a runner

To retire a runner cleanly (host decommission, label rotation, suspected
compromise):

1. On the runner host, stop the service:

   ```sh
   sudo ./svc.sh stop
   sudo ./svc.sh uninstall
   ```

2. Remove it from GitHub's known-runners list. From the host, with a
   removal token:

   ```sh
   ./config.sh remove --token "$REMOVAL_TOKEN"
   ```

   Mint the removal token the same way as the registration token but
   against the `remove-token` endpoint:

   ```sh
   unset GITHUB_TOKEN
   gh api -X POST /repos/raibid-labs/scribe/actions/runners/remove-token --jq .token
   ```

3. Or, if the host is unreachable, force-remove via the GitHub UI:
   <https://github.com/raibid-labs/scribe/settings/actions/runners> →
   the runner row → Remove.

4. Wipe the runner directory:

   ```sh
   rm -rf /home/gh-runner/actions-runner
   ```

5. If you suspect compromise, also rotate any credentials that were
   reachable from the host (SSH keys, model checkpoints if private, etc.)
   and audit recent workflow runs in the Actions tab.

## Troubleshooting

- **Workflow queues forever.** No runner with all four labels is online.
  Check `https://github.com/raibid-labs/scribe/settings/actions/runners`
  — the runner should be **Idle**, not **Offline**. If offline, restart
  the systemd unit on the host.
- **Test still skips.** The test prints the skip reason via `eprintln!`
  before returning. Check the "Run end-to-end integration test" step log
  for a `SKIP transcribe_jfk_end_to_end:` line; it'll name the missing
  dependency.
- **`whisper-cli: command not found`.** Either symlink
  `~/raibid-labs/voice-stuff/third_party/whisper.cpp/build/bin/whisper-cli`
  onto the runner user's `PATH`, or set `SCRIBE_WHISPER_BIN` in the
  runner service environment (see the systemd section above).
- **No model found.** Set `SCRIBE_MODEL_PATH` in the runner environment
  to the directory containing your `ggml-*.bin` files.

## See also

- [`05-contributing.md`](./05-contributing.md) — local dev environment
  setup; the runner host needs the same toolchain.
- [`01-architecture.md`](./01-architecture.md) — what the pipeline
  actually does, so you understand what the integration test is
  validating.
- GitHub: [Security hardening for GitHub Actions](https://docs.github.com/en/actions/security-guides/security-hardening-for-github-actions).
- GitHub: [About self-hosted runners](https://docs.github.com/en/actions/hosting-your-own-runners/managing-self-hosted-runners/about-self-hosted-runners).
