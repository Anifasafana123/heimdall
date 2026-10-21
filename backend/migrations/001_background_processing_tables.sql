-- Migration 001 — Tables for background processing tasks (Issue #5, Safana)
-- PostgreSQL syntax. Append-only where noted — no DELETE/UPDATE on result rows.

-- -------------------------------------------------------------------------
-- offices
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS offices (
    id                  TEXT        NOT NULL PRIMARY KEY,
    name                TEXT        NOT NULL,
    daily_ceiling_bytes BIGINT      NOT NULL DEFAULT 524288000, -- 500 MB
    ceiling_hit         BOOLEAN     NOT NULL DEFAULT FALSE,
    ceiling_reset_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- -------------------------------------------------------------------------
-- devices
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS devices (
    device_id           TEXT        NOT NULL PRIMARY KEY,
    office_id           TEXT        NOT NULL REFERENCES offices(id),
    is_active           BOOLEAN     NOT NULL DEFAULT TRUE,
    last_heartbeat_at   TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_devices_office ON devices(office_id);

-- -------------------------------------------------------------------------
-- pending_results — ingest queue (Task 1)
-- Rows are never deleted; processed=TRUE marks them as consumed.
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS pending_results (
    id          BIGSERIAL   PRIMARY KEY,
    device_id   TEXT        NOT NULL,
    seq         BIGINT      NOT NULL,
    payload     JSONB       NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed   BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_pending_processed ON pending_results(processed, id);

-- -------------------------------------------------------------------------
-- results — append-only (SR-C4)
-- The application role has INSERT + SELECT only — no UPDATE, no DELETE.
-- anomaly_checked and payload_hash support Tasks 1 and 2.
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS results (
    id                  BIGSERIAL   PRIMARY KEY,
    device_id           TEXT        NOT NULL,
    seq                 BIGINT      NOT NULL,
    payload             JSONB       NOT NULL,
    payload_hash        TEXT        NOT NULL,
    latency_ms          DOUBLE PRECISION,
    jitter_ms           DOUBLE PRECISION,
    packet_loss_pct     DOUBLE PRECISION,
    throughput_mbps     DOUBLE PRECISION,
    link_speed_mbps     DOUBLE PRECISION,
    bytes_used          BIGINT      NOT NULL DEFAULT 0,
    anomaly_checked     BOOLEAN     NOT NULL DEFAULT FALSE,
    received_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (device_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_results_device   ON results(device_id);
CREATE INDEX IF NOT EXISTS idx_results_received ON results(received_at);
CREATE INDEX IF NOT EXISTS idx_results_hash     ON results(payload_hash);

-- -------------------------------------------------------------------------
-- quarantine — anomaly flags (Task 2)
-- References results; original row is never touched.
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS quarantine (
    id              BIGSERIAL   PRIMARY KEY,
    result_id       BIGINT      NOT NULL REFERENCES results(id),
    reason          TEXT        NOT NULL,
    quarantined_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed        BOOLEAN     NOT NULL DEFAULT FALSE,
    reviewed_at     TIMESTAMPTZ,
    reviewed_by     TEXT
);

-- -------------------------------------------------------------------------
-- speedtest_leases — one active lease per office (Task 3)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS speedtest_leases (
    id              BIGSERIAL   PRIMARY KEY,
    device_id       TEXT        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending','active','expired','completed')),
    requested_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    granted_at      TIMESTAMPTZ,
    expired_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_lease_status ON speedtest_leases(status);

-- -------------------------------------------------------------------------
-- merkle_roots — daily Merkle tree roots (Task 5)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS merkle_roots (
    id          BIGSERIAL PRIMARY KEY,
    tree_date   DATE      NOT NULL UNIQUE,
    root_hash   TEXT      NOT NULL,
    leaf_count  BIGINT    NOT NULL,
    built_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- -------------------------------------------------------------------------
-- rollups_5min / rollups_1hr / rollups_1day (Task 6)
-- Medians + p10/p90 — never averages (FR22).
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS rollups_5min (
    id                      BIGSERIAL        PRIMARY KEY,
    device_id               TEXT             NOT NULL,
    bucket_start            TIMESTAMPTZ      NOT NULL,
    bucket_end              TIMESTAMPTZ      NOT NULL,
    median_latency_ms       DOUBLE PRECISION,
    p10_latency_ms          DOUBLE PRECISION,
    p90_latency_ms          DOUBLE PRECISION,
    median_throughput_mbps  DOUBLE PRECISION,
    p10_throughput_mbps     DOUBLE PRECISION,
    p90_throughput_mbps     DOUBLE PRECISION,
    median_jitter_ms        DOUBLE PRECISION,
    packet_loss_pct         DOUBLE PRECISION,
    sample_count            INTEGER          NOT NULL DEFAULT 0,
    UNIQUE (device_id, bucket_start)
);

CREATE TABLE IF NOT EXISTS rollups_1hr (
    id                      BIGSERIAL        PRIMARY KEY,
    device_id               TEXT             NOT NULL,
    bucket_start            TIMESTAMPTZ      NOT NULL,
    bucket_end              TIMESTAMPTZ      NOT NULL,
    median_latency_ms       DOUBLE PRECISION,
    p10_latency_ms          DOUBLE PRECISION,
    p90_latency_ms          DOUBLE PRECISION,
    median_throughput_mbps  DOUBLE PRECISION,
    p10_throughput_mbps     DOUBLE PRECISION,
    p90_throughput_mbps     DOUBLE PRECISION,
    median_jitter_ms        DOUBLE PRECISION,
    packet_loss_pct         DOUBLE PRECISION,
    sample_count            INTEGER          NOT NULL DEFAULT 0,
    UNIQUE (device_id, bucket_start)
);

CREATE TABLE IF NOT EXISTS rollups_1day (
    id                      BIGSERIAL        PRIMARY KEY,
    device_id               TEXT             NOT NULL,
    bucket_date             DATE             NOT NULL,
    median_latency_ms       DOUBLE PRECISION,
    p10_latency_ms          DOUBLE PRECISION,
    p90_latency_ms          DOUBLE PRECISION,
    median_throughput_mbps  DOUBLE PRECISION,
    p10_throughput_mbps     DOUBLE PRECISION,
    p90_throughput_mbps     DOUBLE PRECISION,
    median_jitter_ms        DOUBLE PRECISION,
    packet_loss_pct         DOUBLE PRECISION,
    sample_count            INTEGER          NOT NULL DEFAULT 0,
    UNIQUE (device_id, bucket_date)
);

-- -------------------------------------------------------------------------
-- agent_lifecycle_events — SHUTDOWN / BOOT markers (Task 7)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS agent_lifecycle_events (
    id          BIGSERIAL   PRIMARY KEY,
    device_id   TEXT        NOT NULL,
    event       TEXT        NOT NULL CHECK (event IN ('BOOT','SHUTDOWN')),
    event_at    TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_lifecycle_device ON agent_lifecycle_events(device_id, event_at);

-- -------------------------------------------------------------------------
-- connectivity_events — outage / offline timeline (Task 7)
-- Append-only. ended_at is updated when device recovers (not a result row).
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS connectivity_events (
    id              BIGSERIAL   PRIMARY KEY,
    device_id       TEXT        NOT NULL,
    office_id       TEXT        NOT NULL,
    event_type      TEXT        NOT NULL CHECK (event_type IN ('outage','offline')),
    started_at      TIMESTAMPTZ NOT NULL,
    ended_at        TIMESTAMPTZ,
    corroborated    BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_event_device  ON connectivity_events(device_id);
CREATE INDEX IF NOT EXISTS idx_event_office  ON connectivity_events(office_id);
CREATE INDEX IF NOT EXISTS idx_event_started ON connectivity_events(started_at);
