# HEIMDALL — Database Schema

> **Owner:** Yaswini · **Issue:** #3  
> **Database:** PostgreSQL 17  
> **Protocol reference:** `docs/PROTOCOL.md` v1  
> **Timestamps:** all `TIMESTAMPTZ` — UTC

---

## Contents

1. [Design rules](#1-design-rules)
2. [Entity-relationship overview](#2-entity-relationship-overview)
3. [Tables](#3-tables)
   - [offices](#31-offices)
   - [computers](#32-computers)
   - [metrics](#33-metrics)
   - [alerts](#34-alerts)
   - [users](#35-users)
   - [webauthn\_credentials](#36-webauthn_credentials)
   - [audit\_log](#37-audit_log)
   - [corrections](#38-corrections)
4. [Indexes](#4-indexes)
5. [Application roles and permissions](#5-application-roles-and-permissions)
6. [Migrations](#6-migrations)
7. [Connection pool settings](#7-connection-pool-settings)

---

## 1. Design rules

| Rule | Detail |
|---|---|
| Primary keys | `BIGSERIAL` on every table |
| Foreign keys | Named `fk_<child>_<parent>`, enforced by PostgreSQL |
| Timestamps | `TIMESTAMPTZ` UTC on every table |
| Append-only tables | `metrics` and `audit_log` — application role has `INSERT + SELECT` only, no `UPDATE` or `DELETE` |
| Archive | The public archive is kept in full, indefinitely. No rows are ever deleted from `metrics` (SR-C4, PRD §14) |
| Corrections | Corrections are new rows in `corrections` referencing the original — the original always stays |
| No PII | Public IP `/32` never stored — only ASN number. Machine fingerprints are HMAC-hashed |
| Secrets | Office codes stored as Argon2id hashes, never plaintext |
| Envelope fields | `metrics` columns match `docs/PROTOCOL.md` §3 exactly |

---

## 2. Entity-relationship overview

```
offices  1──────< computers >──────1  users (reviewed_by / revoked_by)
   |                  |
   |                  |
   └─────────< metrics >──────────── users (reviewed_by)
                  |
                  └──────────< alerts
                  |                |
                  |             users (resolved_by)
                  |
                  └──────────< corrections ── users (corrected_by)

users 1──────< webauthn_credentials
audit_log  (append-only, references users)
```

---

## 3. Tables

### 3.1 `offices`

One row per registered government institution.

| Column | Type | Notes |
|---|---|---|
| `id` | `BIGSERIAL` PK | — |
| `name` | `VARCHAR(200)` UNIQUE | e.g. *Batticaloa District Secretariat* |
| `province` | `VARCHAR(100)` | Informational |
| `district` | `VARCHAR(100)` | Informational |
| `office_code_hash` | `VARCHAR(255)` | Argon2id hash — never plaintext |
| `max_computers` | `SMALLINT` | Per-office computer limit (FR5) |
| `is_active` | `BOOLEAN` | `FALSE` = suspended |
| `code_revoked_at` | `TIMESTAMPTZ` NULL | Set when code is revoked (FR4) |
| `code_expires_at` | `TIMESTAMPTZ` NULL | Code expiry |
| `created_at` | `TIMESTAMPTZ` | — |
| `updated_at` | `TIMESTAMPTZ` | — |
| `deleted_at` | `TIMESTAMPTZ` NULL | Soft delete |

---

### 3.2 `computers`

One row per enrolled computer. `device_id` equals the certificate serial number — the same value used in the signing envelope (PROTOCOL.md §3).

| Column | Type | Notes |
|---|---|---|
| `id` | `BIGSERIAL` PK | — |
| `office_id` | `BIGINT` FK → `offices.id` | — |
| `device_id` | `VARCHAR(128)` UNIQUE | Certificate serial = envelope `device_id` (PROTOCOL.md §3) |
| `display_label` | `VARCHAR(20)` | Anonymised label e.g. *Computer 3*, randomly assigned (SR-H1) |
| `pubkey_fingerprint` | `CHAR(64)` | SHA-256 hex of Ed25519 public key — clone detection (SR-A2) |
| `machine_fingerprint_hash` | `VARCHAR(255)` | HMAC of hardware IDs — re-enrolment detection (SR-A6) |
| `cert_serial` | `VARCHAR(64)` UNIQUE | Device certificate serial |
| `cert_expires_at` | `TIMESTAMPTZ` NULL | Certificate expiry |
| `last_accepted_seq` | `BIGINT` | Last accepted envelope `seq` — server enforces `seq > this` (SR-C2) |
| `enrolled_at` | `TIMESTAMPTZ` | — |
| `pending_approval_at` | `TIMESTAMPTZ` NULL | `NOT NULL` = held for review (SR-A4) |
| `reviewed_by` | `BIGINT` FK → `users.id` NULL | — |
| `reviewed_at` | `TIMESTAMPTZ` NULL | — |
| `revoked_at` | `TIMESTAMPTZ` NULL | Set on revocation (SR-A5) |
| `revoked_by` | `BIGINT` FK → `users.id` NULL | — |
| `revocation_reason` | `VARCHAR(200)` NULL | — |
| `deleted_at` | `TIMESTAMPTZ` NULL | Soft delete |
| `created_at` | `TIMESTAMPTZ` | — |
| `updated_at` | `TIMESTAMPTZ` | — |

---

### 3.3 `metrics`

**Append-only.** One row per signed envelope received from an agent. Application role: `INSERT + SELECT` only — no `UPDATE`, no `DELETE`.

The archive is kept in full, indefinitely (PRD §14, SR-C4). Corrections are new rows in the `corrections` table — the original row always stays.

#### Signing envelope fields (PROTOCOL.md §3)

| Column | Type | Protocol field | Notes |
|---|---|---|---|
| `device_id` | `VARCHAR(128)` | `device_id` | Certificate serial |
| `seq` | `BIGINT` | `seq` | Strictly increasing per device — UNIQUE with `computer_id` blocks replays (SR-C2) |
| `nonce` | `CHAR(22)` | `nonce` | Base64url 16 random bytes (22 chars) |
| `ts_agent` | `BIGINT` | `ts_agent` | Agent wall-clock Unix seconds — advisory only |
| `clock_skew_secs` | `INTEGER` NULL | — | `server_time − ts_agent` |
| `payload_type` | `VARCHAR(20)` | `payload_type` | One of: `heartbeat`, `measurement`, `failure`, `lease_request` |
| `payload_hash` | `CHAR(43)` | `payload_hash` | Base64url SHA-256 of canonical payload (43 chars) |
| `prev_hash` | `CHAR(43)` | `prev_hash` | Base64url SHA-256 of previous envelope (43 chars) |
| `sig` | `CHAR(86)` | `sig` | Base64url Ed25519 signature (86 chars) |

#### Server timestamp

| Column | Type | Notes |
|---|---|---|
| `recorded_at` | `TIMESTAMPTZ` | Server receive time — **authoritative** for the archive (SR-C2) |

#### Measurement payload fields (PROTOCOL.md §4.4)

| Column | Type | Notes |
|---|---|---|
| `latency_min_ms` | `INTEGER` NULL | — |
| `latency_max_ms` | `INTEGER` NULL | — |
| `latency_mean_ms` | `INTEGER` NULL | — |
| `latency_median_ms` | `INTEGER` NULL | — |
| `latency_p95_ms` | `INTEGER` NULL | — |
| `jitter_mean_ms` | `INTEGER` NULL | — |
| `jitter_max_ms` | `INTEGER` NULL | — |
| `packets_sent` | `INTEGER` NULL | — |
| `packets_received` | `INTEGER` NULL | — |
| `loss_percent` | `DOUBLE PRECISION` NULL | 0–100 |
| `download_bps` | `BIGINT` NULL | Bits per second |
| `upload_bps` | `BIGINT` NULL | Bits per second |
| `throughput_streams` | `SMALLINT` NULL | Parallel TCP streams used |
| `interface_type` | `VARCHAR(20)` NULL | `ethernet`, `wifi`, `cellular`, `loopback`, `other` |
| `link_speed_bps` | `BIGINT` NULL | Negotiated NIC link speed |
| `mtu` | `SMALLINT` NULL | — |
| `dns_resolver_class` | `VARCHAR(10)` NULL | `isp`, `public`, `private`, `unknown` |
| `asn` | `INTEGER` NULL | ASN only — `/32` address never stored (PR4) |
| `probe_id` | `VARCHAR(64)` NULL | HEIMDALL probe node used |
| `uptime_secs` | `BIGINT` NULL | Heartbeat payload field |

#### Quality flags

| Value | Meaning |
|---|---|
| `ok` | Passed validation — in the public record |
| `quarantine` | Held for maintainer review (SR-C3) |
| `approved` | Maintainer approved a quarantined row |
| `rejected` | Excluded from public view |

---

### 3.4 `alerts`

System-generated anomaly and security alerts.

| Column | Type | Notes |
|---|---|---|
| `id` | `BIGSERIAL` PK | — |
| `office_id` | `BIGINT` FK NULL | NULL for global alerts |
| `computer_id` | `BIGINT` FK NULL | — |
| `metric_id` | `BIGINT` FK NULL | The metric that triggered the alert |
| `alert_type` | `VARCHAR(60)` | e.g. `clone_detected`, `enrol_limit_exceeded`, `off_site`, `replay_attempt` |
| `severity` | `VARCHAR(10)` | `info`, `warning`, `critical` |
| `description` | `VARCHAR(500)` | System-generated message |
| `status` | `VARCHAR(20)` | `open`, `ack`, `resolved`, `false_positive` |
| `resolved_by` | `BIGINT` FK → `users.id` NULL | — |
| `resolved_at` | `TIMESTAMPTZ` NULL | — |
| `resolution_note` | `VARCHAR(500)` NULL | — |
| `created_at` | `TIMESTAMPTZ` | — |
| `updated_at` | `TIMESTAMPTZ` | — |

---

### 3.5 `users`

HEIMDALL maintainer accounts — not office staff.

Roles are stored as a `TEXT[]` array. Valid values: `institution_approver`, `device_manager`, `release_publisher`, `data_reviewer`, `auditor`.

| Column | Type | Notes |
|---|---|---|
| `id` | `BIGSERIAL` PK | — |
| `username` | `VARCHAR(60)` UNIQUE | — |
| `email` | `VARCHAR(254)` UNIQUE | — |
| `password_hash` | `VARCHAR(255)` | bcrypt hash |
| `roles` | `TEXT[]` | Default `{auditor}` |
| `is_active` | `BOOLEAN` | `FALSE` = suspended |
| `last_login_at` | `TIMESTAMPTZ` NULL | — |
| `created_at` | `TIMESTAMPTZ` | — |
| `updated_at` | `TIMESTAMPTZ` | — |
| `deleted_at` | `TIMESTAMPTZ` NULL | Soft delete |

---

### 3.6 `webauthn_credentials`

WebAuthn hardware key registrations per maintainer (SR-G1).

| Column | Type | Notes |
|---|---|---|
| `id` | `BIGSERIAL` PK | — |
| `user_id` | `BIGINT` FK → `users.id` | — |
| `credential_id` | `VARCHAR(1024)` | Base64url credential ID |
| `public_key` | `TEXT` | CBOR-encoded public key, base64url |
| `aaguid` | `CHAR(36)` | Authenticator model identifier |
| `sign_count` | `BIGINT` | Increments each use — detects cloned authenticators |
| `friendly_name` | `VARCHAR(100)` | e.g. *YubiKey 5 NFC* |
| `registered_at` | `TIMESTAMPTZ` | — |
| `last_used_at` | `TIMESTAMPTZ` NULL | — |

---

### 3.7 `audit_log`

**Append-only, hash-chained.** Every maintainer action is recorded here (SR-G3). Application role: `INSERT + SELECT` only.

| Column | Type | Notes |
|---|---|---|
| `id` | `BIGSERIAL` PK | — |
| `user_id` | `BIGINT` FK NULL | NULL for system events |
| `ip_hash` | `VARCHAR(64)` NULL | SHA-256 of admin IP — never raw (PR4) |
| `action` | `VARCHAR(80)` | e.g. `office.approve`, `computer.revoke`, `release.publish` |
| `target_type` | `VARCHAR(40)` NULL | e.g. `office`, `computer`, `metric` |
| `target_id` | `BIGINT` NULL | PK of the affected row |
| `details` | `JSONB` NULL | Structured change details — no PII |
| `prev_hash` | `CHAR(64)` NULL | SHA-256 of previous row (NULL for row 1) |
| `row_hash` | `CHAR(64)` | SHA-256 of this row's content |
| `created_at` | `TIMESTAMPTZ` | — |

---

### 3.8 `corrections`

Corrections are new rows — the original metric row always stays (SR-C4). The archive is never silently rewritten.

| Column | Type | Notes |
|---|---|---|
| `id` | `BIGSERIAL` PK | — |
| `original_id` | `BIGINT` FK → `metrics.id` | The original row — never deleted |
| `corrected_by` | `BIGINT` FK → `users.id` | The maintainer who made the correction |
| `corrected_at` | `TIMESTAMPTZ` | — |
| `reason` | `VARCHAR(500)` | Why the correction was made |
| `detail` | `JSONB` | What changed and why |

---

## 4. Indexes

### `metrics` indexes

| Index | Columns | Serves |
|---|---|---|
| `PRIMARY` | `id` | — |
| `uq_metrics_device_seq` | `(computer_id, seq)` UNIQUE | Replay prevention — DB-level guarantee (SR-C2) |
| `idx_metrics_computer_time` | `(computer_id, recorded_at DESC)` | General computer time-range scan |
| `idx_metrics_office_time` | `(office_id, recorded_at DESC)` | General office time-range scan |
| `idx_metrics_latest_per_computer` | `(computer_id, payload_type, quality, recorded_at DESC)` | Fetch latest metric — single row, no sort |
| `idx_metrics_history_range` | `(computer_id, payload_type, recorded_at, quality)` | History by time range — chart data |
| `idx_metrics_office_summary` | `(office_id, quality, recorded_at, payload_type)` | Office dashboard aggregation |
| `idx_metrics_office_last_seen` | `(quality, payload_type, office_id, recorded_at DESC)` | Public map live pin |
| `idx_metrics_outages` | `(office_id, payload_type, recorded_at, computer_id)` | Failure/outage history |
| `idx_metrics_payload_hash` | `(payload_hash)` | Merkle proof lookup |

### `computers` indexes

| Index | Columns | Serves |
|---|---|---|
| `uq_computers_device_id` | `(device_id)` UNIQUE | Device lookup by cert serial |
| `uq_computers_cert_serial` | `(cert_serial)` UNIQUE | Cert validation |
| `idx_computers_office_id` | `(office_id)` | List computers for an office |
| `idx_computers_pubkey_fp` | `(pubkey_fingerprint)` | Clone detection |
| `idx_computers_pending` | `(pending_approval_at, revoked_at, deleted_at)` WHERE pending | Pending review queue |
| `idx_computers_office_active` | `(office_id, revoked_at, deleted_at)` WHERE active | Active count check (FR5) |

### `alerts` indexes

| Index | Columns | Serves |
|---|---|---|
| `idx_alerts_office` | `(office_id)` | — |
| `idx_alerts_computer` | `(computer_id)` | — |
| `idx_alerts_type_status` | `(alert_type, status)` | Filter by type and state |
| `idx_alerts_created` | `(created_at DESC)` | Recent alerts |
| `idx_alerts_office_open` | `(office_id, status, created_at DESC)` WHERE open | Open alerts for an office |

### `audit_log` indexes

| Index | Columns | Serves |
|---|---|---|
| `idx_audit_user` | `(user_id)` | — |
| `idx_audit_action` | `(action)` | Filter by action type |
| `idx_audit_target` | `(target_type, target_id)` | All actions on one entity |
| `idx_audit_created` | `(created_at DESC)` | Chronological log |
| `idx_audit_user_time` | `(user_id, created_at DESC)` | Recent actions by a maintainer |

---

## 5. Application roles and permissions

```sql
-- Read/write role for the backend
CREATE USER heimdall_app WITH PASSWORD '<strong-password>';
GRANT SELECT, INSERT ON metrics    TO heimdall_app;
GRANT SELECT, INSERT ON audit_log  TO heimdall_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON offices              TO heimdall_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON computers            TO heimdall_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON alerts               TO heimdall_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON users                TO heimdall_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON webauthn_credentials TO heimdall_app;
GRANT SELECT, INSERT                 ON corrections          TO heimdall_app;

-- Read-only role for the public read API
CREATE USER heimdall_ro WITH PASSWORD '<strong-password>';
GRANT SELECT ON metrics   TO heimdall_ro;
GRANT SELECT ON offices   TO heimdall_ro;
GRANT SELECT ON computers TO heimdall_ro;
```

> `metrics` and `audit_log` must never receive `UPDATE` or `DELETE` grants.
> The append-only guarantee is enforced at the role level, not just in code.

---

## 6. Migrations

Migrations live in `backend/migrations/` and run automatically at startup via SQLx.

| File | Description |
|---|---|
| `001_initial_schema.sql` | All 8 tables, foreign keys, indexes |

### Running manually

```bash
cd backend
cargo sqlx migrate run

# Check status
cargo sqlx migrate info
```

### Regenerating the offline query cache

After changing any `sqlx::query!` or `sqlx::query_as!` call:

```bash
cd backend
cargo sqlx prepare
# commit the updated .sqlx/ directory
```

---

## 7. Connection pool settings

Configured via environment variables (see `backend/.env.example`):

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | — | **Required.** PostgreSQL connection URL |
| `DATABASE_MAX_CONNECTIONS` | `10` | Maximum open connections |
| `DATABASE_MIN_CONNECTIONS` | `2` | Minimum idle connections |
| `DATABASE_CONNECT_TIMEOUT_SECS` | `30` | Seconds to wait for a connection |
| `DATABASE_IDLE_TIMEOUT_SECS` | `600` | Seconds before idle connection closed |
| `DATABASE_MAX_LIFETIME_SECS` | `1800` | Seconds before connection recycled |
