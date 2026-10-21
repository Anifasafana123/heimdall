# API Contract — Network Monitoring System (Backend REST API)

Base URL: `https://<host>:<port>`

All request and response bodies are JSON unless noted otherwise (`text/html` for HTMX dashboard views).

---

## Authentication

Admin API endpoints (under `/api/...`) require a JWT bearer token obtained from `/auth/login`.

```
Authorization: Bearer <token>
```

The agent ingest endpoint (`/api/agents/ingest`) is authenticated by verifying the computer is registered and authorized (no bearer token required).

---

## Endpoints

### `GET /health`

**Response** `200`
```json
{ "status": "ok" }
```

### `POST /auth/login`

Authenticate an admin user.

**Request**
```json
{ "username": "admin", "password": "..." }
```

**Response** `200`
```json
{ "token": "<jwt>", "username": "admin" }
```

**Errors** `401` invalid credentials.

---

### `GET /api/computers`  *(auth)*

List all registered computers.

**Response** `200` — array of `Computer`:
```json
[
  {
    "id": "uuid-string",
    "hostname": "workstation-01",
    "mac_address": "AA:BB:CC:DD:EE:01",
    "ip_address": "192.168.1.10",
    "owner": "Simha",
    "is_authorized": true,
    "registered_at": "2026-01-01T00:00:00Z",
    "last_seen_at": null
  }
]
```

### `POST /api/computers`  *(auth)*

Register a new computer.

**Request**
```json
{ "hostname": "workstation-02", "mac_address": "AA:BB:CC:DD:EE:02", "ip_address": "192.168.1.11", "owner": "Robeeta" }
```

**Response** `200` — the created `Computer`.

### `GET /api/computers/{id}`  *(auth)*

Get a single computer. `404` if not found.

### `POST /api/computers/{id}/authorize/{authorized}`  *(auth)*

Set the authorization flag. `authorized` is `true` or `false`.

**Response** `200` — the updated `Computer`.

---

### `POST /api/agents/ingest`

Used by the local monitoring agent to submit a batch of metrics.

**Request**
```json
{
  "computer_id": "uuid-string",
  "metrics": [
    { "metric_kind": "cpu_usage", "value": 42.5, "recorded_at": "2026-01-01T00:00:00Z" },
    { "metric_kind": "memory_usage", "value": 71.0, "recorded_at": "2026-01-01T00:00:00Z" }
  ]
}
```

**Response** `200`
```json
{ "stored": 2 }
```

**Errors**
- `404` computer not found.
- `403` computer is not authorized.
- Unapproved `metric_kind` values are rejected individually (skipped, not stored).

### `GET /api/computers/{id}/metrics`  *(auth)*

Read metrics for a computer, optionally filtered.

**Query parameters**

| param         | type     | description                         |
|---------------|----------|-------------------------------------|
| `metric_kind` | string   | filter by metric kind               |
| `from`        | datetime | ISO 8601, inclusive lower bound     |
| `to`          | datetime | ISO 8601, inclusive upper bound     |
| `limit`       | integer  | max results, default 500, max 5000 |

**Response** `200` — array of `Metric`:
```json
[
  { "id": 1, "computer_id": "uuid-string", "metric_kind": "cpu_usage", "value": 42.5, "recorded_at": "2026-01-01T00:00:00Z" }
]
```

---

## Approved Metric Kinds

Only these metric kinds are accepted by `/api/agents/ingest`:

| kind           | unit | description                |
|----------------|------|----------------------------|
| `cpu_usage`    | %    | CPU utilization            |
| `memory_usage` | %    | Memory utilization         |
| `disk_usage`   | %    | Disk utilization           |
| `network_rx`   | Mbps | Network receive throughput |
| `network_tx`   | Mbps | Network transmit throughput|

---

## HTMX Dashboard Views (HTML)

Server-rendered with MiniJinja templates and served by this backend.

| Method | Path                      | Description                                    |
|--------|---------------------------|------------------------------------------------|
| GET    | `/`                       | Dashboard page (computers + metrics panels)    |
| GET    | `/login`                  | Login form                                      |
| POST   | `/login`                  | Login submit (sets cookie, redirects)          |
| GET    | `/partials/computers`     | HTMX partial: computers table (auto-refresh)    |
| GET    | `/partials/metrics/{id}`  | HTMX partial: recent metrics for a computer     |

---

## Data Types

- `uuid-string` — RFC 4122 UUID v4 stored as `CHAR(36)`.
- `datetime` — ISO 8601 / RFC 3339 UTC, e.g. `2026-01-01T00:00:00Z`.
- `value` — `DOUBLE` floating point.
