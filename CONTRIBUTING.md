# Contributing to HEIMDALL

The spec is [HEIMDALL-PRD.md](HEIMDALL-PRD.md). `TECHNICAL-PLAN.md` explains
how it will be built. Both are starting points, so push back where you
disagree. Section 11 of the PRD (security) is not open to redesign.

If an issue contradicts the PRD, say so on the issue instead of building it.

## Who works on what

Issues are being rewritten against the PRD. Until then, the work below is
proposed. Do not start code that depends on the stack or the schema before the
maintainer confirms them.

| Issue | Area | Contributor | Proposed scope |
|---|---|---|---|
| #1 | Agent | Simha | Network measurements only: speed, latency, jitter, loss, uptime (FR10). Keep recording through outages (FR12). A failed measurement is still a result (FR13). Tell a lost connection from a switched-off computer (FR14). Outbound only, runs no commands (SR-E1, SR-E2). SQLite spool for results during an outage. |
| #2 | Backend API | Robeeta | Enrolment with office codes and per-device identity (FR1 to FR6, SR-A). Ingest endpoint for signed results. Public read API for the map and archive. Abuse limits (SR-G6). Maintainer admin stays a separate service. |
| #3 | Database | Yaswini | PostgreSQL schema from the PRD: offices, office_codes, devices, enrolment_events, presence_state, results, quarantine, corrections, audit_log, merkle_roots. Insert and select only for the application role (SR-C4). Medians with p10 and p90 (FR22). No full IPs, no precise locations (PR4). No delete jobs. |
| #4 | Frontend | Farwes | HTMX public map and archive, no account needed (FR17 to FR24). Anonymised per-computer view (FR20). Office coverage (FR6a). Enrolment changes (FR21). Admin views are secondary. |
| #5 | Background jobs | Safana | Rollups and aggregates (5 minutes, 1 hour, 1 day). Silence detection so offices without reports still appear (FR23). Quarantine of suspicious values (SR-C3). Daily Merkle root for public verification (SR-C5). No retention deletes. No alerts. |
| #6 | Web activity | Kumar | On hold. Issue #6 as written is rejected because it breaks PR2 and PR3. A narrow form (category counts per office per day, optional component) needs a PRD amendment first. Wait for the maintainer. |

Do not build any of these:

- A job that deletes old results. The archive is kept in full, indefinitely.
- CPU, memory or disk telemetry.
- Alerts. HEIMDALL publishes, it does not alert.
- Browser history, URLs, page titles or any per-person activity.

## Workflow

Work from a fork, locally, never through the GitHub web upload. Never work on
your fork's `main`.

One-time setup:

```bash
git clone https://github.com/<your-username>/heimdall.git
cd heimdall
git remote add upstream https://github.com/Rumeasiyan/heimdall.git
```

For every new piece of work:

```bash
git checkout main
git fetch upstream
git reset --hard upstream/main
git checkout -b feat/short-name
```

Then commit, push the branch to your fork and open a pull request from
`<your-username>:feat/short-name` to `Rumeasiyan:main`.

Branch names use `type/short-name`, for example `docs/readme`,
`feat/enrolment` or `fix/outage-spool`.

## Commit messages

Use Conventional Commits. Squash each pull request to one commit.

```text
<type>(<scope>): <imperative summary, lower case, no full stop, 50 chars max>

<body: why, not what, wrapped at 72 characters>

Refs: #N
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `build`, `ci`,
`perf`.

The subject must complete the sentence "If applied, this commit will ...".

| Avoid | Use |
|---|---|
| `Add files via upload` | `docs: add README and contributing guide` |
| `Updated stuff` | `fix: reject enrolment past the office limit` |

To squash before merging:

```bash
git rebase -i upstream/main
git push --force-with-lease
```

## Pull requests

- The title follows the commit format.
- The description matches the diff. Update it whenever the diff changes.
- Reference the issue, for example `Closes #N`.
- Do not push directly to `main`.
- End every file with a newline, and leave a blank line before each heading.

## Privacy and security

HEIMDALL never collects browsing history, page titles, URLs, files,
keystrokes or the identity of any user. Report vulnerabilities privately to
the maintainers, not in public issues.
