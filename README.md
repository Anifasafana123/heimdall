# HEIMDALL

> **The guard at the bridge.**

HEIMDALL is an open-source, public connectivity monitor for government
institutions. It runs on computers in government offices, measures the real
internet connection those offices work on, and publishes the results openly on
a **public map**, live and as a historical archive, so anyone can see how well
the government is connected.

It follows the example of [Giga](https://maps.giga.global/map), the
UNICEF–ITU initiative that maps school connectivity worldwide. HEIMDALL
applies the same idea to government offices.

**Licence:** MIT. **Status:** pre-pilot. The pilot launches on
16 September 2026 in the Eastern Province of Sri Lanka.

## Why

Government services depend on a working internet connection, yet nobody can
see which offices actually have one. Problems surface only when someone
reports them, and offices with failing connections that never report stay
invisible. HEIMDALL makes that picture public.

## How it works

```text
Office computers                Central internet service              Public
┌──────────────┐  HTTPS       ┌────────────────────────────┐      ┌────────────┐
│ HEIMDALL     │ ───────────► │ Ingest API                 │      │ Public map │
│ agent        │  signed      │ Database (append-only)     │ ───► │ + archive  │
│ (measures)   │  results     │ Background jobs            │      │ no account │
└──────────────┘              │ Maintainer admin (separate)│      └────────────┘
 outbound only                └────────────────────────────┘
```

1. An institution sends a formal letter to the maintainers.
2. The maintainers approve it and issue an office code.
3. Staff install the agent on office computers and enter the code once. Each
   computer then gets its own identity.
4. The agent shows what it measures and what it never touches, then runs
   quietly in the background.
5. It measures speed, latency, jitter, packet loss and uptime, keeps recording
   through outages, and sends signed results to the service.
6. The service checks the results, stores them in an append-only archive and
   publishes them on the public map.

## What it does

- Measures real connectivity continuously: download and upload speed,
  response time, connection stability and availability.
- Publishes a public map and a historical archive that anyone can view
  without an account.
- Shows every participating office and every computer, including those that
  have stopped reporting. Silence is shown as silence.
- Counts a result for an office only when the computer is actually at that
  office.
- Describes a connection by typical performance (median, with p10 and p90),
  never by its best moment.
- Stays light on the office connection: tiny regular signals, short and
  infrequent speed tests, one test at a time per office, and a daily data
  ceiling.

## Privacy boundary

HEIMDALL measures the connection and nothing else. These rules are also shown
inside the app, in plain language.

- **PR1.** HEIMDALL measures only the network connection and the basic
  connection settings of the computer it runs on.
- **PR2.** It never collects files, documents, browsing history, keystrokes,
  screen content, or the identity of the person using the computer.
- **PR3.** It never publishes anything that identifies a staff member or a
  specific computer. Public per-computer data is anonymised.
- **PR4.** It does not keep full network addresses or precise locations, and
  publishes neither.
- **PR5.** The privacy boundary is shown inside the app itself, in plain
  language, not only in a separate document.

## Security

Security is the highest-priority requirement. HEIMDALL runs on government
computers and publishes a record the public will rely on.

- The agent only reaches out. Nothing can connect in, and it never runs
  commands sent to it.
- Every computer has its own identity. An office code alone can never submit
  results.
- Results are tamper-evident from the moment they are measured.
- The public archive is append-only. Nothing is silently changed or deleted.
  Corrections are new, dated rows that reference the original.
- Every release can be verified as genuine before installation.

The full threat model is section 11 of [HEIMDALL-PRD.md](HEIMDALL-PRD.md).

## Technology

| Part | Choice |
|---|---|
| Language | Rust |
| Frontend | HTMX |
| Central database | PostgreSQL, with TimescaleDB planned before national scale |
| Agent local storage | SQLite, for results waiting during an outage |

Details and costs are in `TECHNICAL-PLAN.md`. Where this table and that
document differ, that document wins.

## Planned layout

This layout is planned, not final. The folders below do not all exist yet.

```text
agent/        measuring agent that runs on office computers
backend/      ingest API, public API and background jobs
  migrations/ database migrations
web/          HTMX public map and archive
docs/         database and API documentation
```

## Documentation

| Document | Purpose |
|---|---|
| [HEIMDALL-PRD.md](HEIMDALL-PRD.md) | What HEIMDALL must do and why. The spec. |
| `TECHNICAL-PLAN.md` | How, when and what it costs. |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Who works on what, and how to commit. |

## Roadmap

1. **Pilot:** Eastern Province, 16 September 2026.
2. **After the pilot:** other provinces, through their Chief Secretaries.
3. **Longer term:** national coverage, with nothing preventing adoption
   elsewhere.

## Contributing

Collaboration and support are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md)
and the PRD first.
