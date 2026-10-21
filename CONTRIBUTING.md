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
| #6 | Web activity | Niroshmaynan (@Niroshmaynan) | On hold. Issue #6 as written is rejected because it breaks PR2 and PR3. A narrow form (category counts per office per day, optional component) needs a PRD amendment first. Wait for the maintainer. |

Do not build any of these:

- A job that deletes old results. The archive is kept in full, indefinitely.
- CPU, memory or disk telemetry.
- Alerts. HEIMDALL publishes, it does not alert.
- Browser history, URLs, page titles or any per-person activity.

## Working agreement

Four rules apply to every pull request:

| # | Rule |
|---|---|
| 1 | **Every PR traces to an issue.** If the work is not described in an issue, the PR is rejected and the issue gets opened first |
| 2 | **Stay inside your issue.** Work belonging to someone else's issue, or to no issue, comes out of the PR |
| 3 | **Respect the order.** A PR that jumps a dependency is rejected until the earlier step lands |
| 4 | **Link both ways, and close what is dead.** The PR says `Closes #N`. The issue gets a comment pointing at the PR. A PR that is superseded or needs rebuilding gets closed by its author with a one-line reason, not left open |

Opening issues is the contributor's responsibility. If something needs building and no issue covers it, open the issue and say why rather than building it quietly.

### Build order

```text
1. Protocol specification      #15  <- start here. Everything depends on it
2. Database schema             #3
3. Backend API                 #2
4. Agent                       #1
5. Background jobs             #5
6. Dashboard and public map    #4 admin console, #16 public map
```

Step 1 defines the wire format: message shapes, the signing envelope, error codes. Right now the agent, the backend and the database each invent their own and they do not match. Nothing above it can be correct until it exists.

### Three habits for linking and PR hygiene

| # | Habit |
|---|---|
| 1 | **The PR names the issue.** `Closes #N` in the description, so merging the PR closes the issue automatically |
| 2 | **The issue names the PR.** Comment on the issue when you open a PR for it, so anyone reading the issue can find the work |
| 3 | **Close what is dead.** A PR that has been superseded, abandoned, or needs rebuilding from a fresh branch gets closed by its author, with a one-line reason. Do not leave it open "just in case" |

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
keystrokes or the identity of any user.

### Reporting vulnerabilities (SR-I1)

Do not report security vulnerabilities in public issues.

Use GitHub's **Private Vulnerability Reporting** for this repository: navigate
to the **Security** tab, select **Advisories**, and click **Report a vulnerability**.

- **Acknowledgement:** We acknowledge all security reports within 48 hours.
- **Good-faith researchers:** Good-faith security researchers acting in accordance with responsible disclosure practices are welcome.
- **Full policy:** The complete vulnerability disclosure policy and threat model will be documented in `SECURITY.md` (#13).
