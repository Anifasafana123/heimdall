# Network Monitoring System

A system that monitors authorized computers on a network and shows approved system and network metrics on an admin dashboard.

> Monitoring is limited to authorized computers and approved system/network metrics.

## Architecture

```
Registered Computers (01, 02, 03, 04, others)
        |  Local Monitoring Agent (Rust) -> JSON
        |  LAN / Wi-Fi, HTTPS / TLS
        v
MAIN COMPUTER - CENTRAL SERVER
   Backend REST API (Rust, Axum, Tokio, Serde JSON, Tower HTTP, Tracing)
   Background Processing (Tokio async tasks)
   Database (MySQL Community Edition, SQLx)
   Admin Dashboard (HTMX, Tailwind CSS)
        ^
        |  HTTPS / TLS
Administrator Computer (Web browser)
```

## Technology Stack

| Layer | Technology |
|---|---|
| Agent | Rust, Serde JSON |
| Backend | Rust, Axum, Tokio, Tower HTTP, Tracing |
| Database | MySQL Community Edition, SQLx |
| Background jobs | Tokio async tasks |
| Frontend | HTMX, Tailwind CSS |

## Project Structure

```
.
├── agent/            # Local Monitoring Agent (Rust)
│   └── src/browser_history.rs
├── backend/          # REST API (Rust, Axum)
│   ├── migrations/   # MySQL migrations
│   └── src/tasks/    # Background processing
├── dashboard/        # Admin Dashboard (HTMX, Tailwind CSS)
├── docs/             # Documentation (database, API contract)
└── README.md
```

## Team and Responsibilities

| Issue | Module | Developer | Folder |
|---|---|---|---|
| #1 | Local Monitoring Agent | Simha | `agent/` |
| #2 | Backend REST API | Robeeta | `backend/` |
| #3 | Database (MySQL + SQLx) | Yaswini | `backend/migrations/`, `docs/` |
| #4 | Admin Dashboard | Farwes | `dashboard/` |
| #5 | Background Processing | Safana | `backend/src/tasks/` |
| #6 | Browser History Monitoring | Kumar | `agent/src/browser_history.rs` |

## Getting Started

### Prerequisites

- Rust (stable) and Cargo
- MySQL Community Edition

### Agent

```bash
cd agent
cargo run
```

### Backend

```bash
cd backend
cp .env.example .env     # set DATABASE_URL
cargo run
```

### Dashboard

The dashboard is built with HTMX and served by the backend. Start the backend to access it.
## Contribution Workflow

1. Fork the repository and clone your fork.
2. Pick your assigned issue and create a branch:
   ```bash
   git checkout -b feature/background-processing
   ```
3. Work only inside your assigned folder to avoid merge conflicts.
4. Commit with the issue number:
   ```bash
   git commit -m "Add CPU metrics collection (#1)"
   ```
5. Push the branch and open a Pull Request to the main repository.
6. Write `Closes #5` (your issue number) in the PR description.
7. Do not push directly to `main`.

## Privacy and Authorization

- Only registered and authorized computers send data.
- Only approved metrics are collected.
- Browser history monitoring is opt-in, shows a notice on the monitored computer, is admin-only and has a data retention limit.

## License

See Pending Decisions below.

## Pending Decisions

- [ ] License for the project (to be chosen by the project owner)
- [ ] Template engine for HTMX responses (Askama vs Maud)
- [ ] Whether Tailwind CSS is built with npm or the standalone CLI
- [ ] Authentication method for agents (API key vs mTLS)
- [ ] Data retention period for browser history
- [ ] How often agents send data to the backend