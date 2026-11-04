-- Migration 001 — PostgreSQL 17 Tables required by Safana's background processing tasks (Issue #5)
-- Run with: sqlx migrate run

-- -------------------------------------------------------------------------
-- offices
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS offices (
    id                  VARCHAR(36)  NOT NULL PRIMARY KEY,
    name                VARCHAR(255) NOT NULL,
    daily_ceiling_bytes BIGINT       NOT NULL DEFAULT 524288000, -- 500 MB
    ceiling_hit         BOOLEAN      NOT NULL DEFAULT FALSE,
    ceiling_reset_at    TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- -------------------------------------------------------------------------
-- devices
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS devices (
    device_id           VARCHAR(36)  NOT NULL PRIMARY KEY,
    office_id           VARCHAR(36)  NOT NULL,
    is_active           BOOLEAN      NOT NULL DEFAULT TRUE,
    last_heartbeat_at   TIMESTAMPTZ  NULL,
    last_known_seq      BIGINT       NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_device_office FOREIGN KEY (office_id) REFERENCES offices(id)
);

-- -------------------------------------------------------------------------
-- pending_results  — ingest queue (Task 1)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS pending_results (
    id              BIGSERIAL    PRIMARY KEY,
    device_id       VARCHAR(36)  NOT NULL,
    seq             BIGINT       NOT NULL,
    payload         JSONB        NOT NULL,
    received_at     TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at    TIMESTAMPTZ  NULL
);

CREATE INDEX IF NOT EXISTS idx_pending_device ON pending_results (device_id);
CREATE INDEX IF NOT EXISTS idx_pending_received ON pending_results (received_at);

-- -------------------------------------------------------------------------
-- results  — append-only; past results are never silently changed or deleted (SR-C4)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS results (
    id                  BIGSERIAL        PRIMARY KEY,
    device_id           VARCHAR(36)      NOT NULL,
    seq                 BIGINT           NOT NULL,
    payload             JSONB            NOT NULL,
    payload_hash        VARCHAR(64)      NOT NULL,  -- SHA-256 hex
    latency_ms          DOUBLE PRECISION NULL,
    jitter_ms           DOUBLE PRECISION NULL,
    packet_loss_pct     DOUBLE PRECISION NULL,
    throughput_mbps     DOUBLE PRECISION NULL,
    link_speed_mbps     DOUBLE PRECISION NULL,
    bytes_used          BIGINT           NOT NULL DEFAULT 0,
    anomaly_checked     BOOLEAN          NOT NULL DEFAULT FALSE,
    received_at         TIMESTAMPTZ      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_device_seq UNIQUE (device_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_results_device   ON results (device_id);
CREATE INDEX IF NOT EXISTS idx_results_received ON results (received_at);
CREATE INDEX IF NOT EXISTS idx_results_hash     ON results (payload_hash);

-- -------------------------------------------------------------------------
-- quarantine  — rows flagged by anomaly detector (Task 2 / SR-C3)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS quarantine (
    id              BIGSERIAL    PRIMARY KEY,
    result_id       BIGINT       NOT NULL,
    reason          VARCHAR(255) NOT NULL,
    quarantined_at  TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reviewed        BOOLEAN      NOT NULL DEFAULT FALSE,
    reviewed_at     TIMESTAMPTZ  NULL,
    reviewed_by     VARCHAR(255) NULL,
    CONSTRAINT fk_quarantine_result FOREIGN KEY (result_id) REFERENCES results(id)
);

-- -------------------------------------------------------------------------
-- speedtest_leases  — one active lease per office (Task 3 / FR30)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS speedtest_leases (
    id              BIGSERIAL    PRIMARY KEY,
    device_id       VARCHAR(36)  NOT NULL,
    status          VARCHAR(20)  NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'active', 'expired', 'completed')),
    requested_at    TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
    granted_at      TIMESTAMPTZ  NULL,
    expired_at      TIMESTAMPTZ  NULL
);

CREATE INDEX IF NOT EXISTS idx_lease_device ON speedtest_leases (device_id);
CREATE INDEX IF NOT EXISTS idx_lease_status ON speedtest_leases (status);

-- -------------------------------------------------------------------------
-- merkle_roots  — daily Merkle tree roots for public verification (Task 5 / SR-C5)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS merkle_roots (
    id              BIGSERIAL    PRIMARY KEY,
    tree_date       DATE         NOT NULL UNIQUE,
    root_hash       VARCHAR(64)  NOT NULL,  -- SHA-256 hex
    leaf_count      BIGINT       NOT NULL,
    built_at        TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- -------------------------------------------------------------------------
-- rollups_5min  — 5-minute percentiles (Task 6 / FR22)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS rollups_5min (
    id                      BIGSERIAL        PRIMARY KEY,
    device_id               VARCHAR(36)      NOT NULL,
    bucket_start            TIMESTAMPTZ      NOT NULL,
    bucket_end              TIMESTAMPTZ      NOT NULL,
    median_latency_ms       DOUBLE PRECISION NULL,
    p10_latency_ms          DOUBLE PRECISION NULL,
    p90_latency_ms          DOUBLE PRECISION NULL,
    median_throughput_mbps  DOUBLE PRECISION NULL,
    p10_throughput_mbps     DOUBLE PRECISION NULL,
    p90_throughput_mbps     DOUBLE PRECISION NULL,
    median_jitter_ms        DOUBLE PRECISION NULL,
    packet_loss_pct         DOUBLE PRECISION NULL,
    sample_count            BIGINT           NOT NULL DEFAULT 0,
    CONSTRAINT uq_rollup_5m_device_bucket UNIQUE (device_id, bucket_start)
);

-- -------------------------------------------------------------------------
-- rollups_1hr  — 1-hour percentiles (Task 6 / FR22)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS rollups_1hr (
    id                      BIGSERIAL        PRIMARY KEY,
    device_id               VARCHAR(36)      NOT NULL,
    bucket_start            TIMESTAMPTZ      NOT NULL,
    bucket_end              TIMESTAMPTZ      NOT NULL,
    median_latency_ms       DOUBLE PRECISION NULL,
    p10_latency_ms          DOUBLE PRECISION NULL,
    p90_latency_ms          DOUBLE PRECISION NULL,
    median_throughput_mbps  DOUBLE PRECISION NULL,
    p10_throughput_mbps     DOUBLE PRECISION NULL,
    p90_throughput_mbps     DOUBLE PRECISION NULL,
    median_jitter_ms        DOUBLE PRECISION NULL,
    packet_loss_pct         DOUBLE PRECISION NULL,
    sample_count            BIGINT           NOT NULL DEFAULT 0,
    CONSTRAINT uq_rollup_1h_device_bucket UNIQUE (device_id, bucket_start)
);

-- -------------------------------------------------------------------------
-- rollups_1day  — 1-day percentiles (Task 6 / FR22)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS rollups_1day (
    id                      BIGSERIAL        PRIMARY KEY,
    device_id               VARCHAR(36)      NOT NULL,
    bucket_date             DATE             NOT NULL,
    median_latency_ms       DOUBLE PRECISION NULL,
    p10_latency_ms          DOUBLE PRECISION NULL,
    p90_latency_ms          DOUBLE PRECISION NULL,
    median_throughput_mbps  DOUBLE PRECISION NULL,
    p10_throughput_mbps     DOUBLE PRECISION NULL,
    p90_throughput_mbps     DOUBLE PRECISION NULL,
    median_jitter_ms        DOUBLE PRECISION NULL,
    packet_loss_pct         DOUBLE PRECISION NULL,
    sample_count            BIGINT           NOT NULL DEFAULT 0,
    CONSTRAINT uq_rollup_1d_device_bucket UNIQUE (device_id, bucket_date)
);

-- -------------------------------------------------------------------------
-- connectivity_events  — outage vs offline tracking (Task 7 / FR14)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS connectivity_events (
    id              BIGSERIAL    PRIMARY KEY,
    device_id       VARCHAR(36)  NOT NULL,
    office_id       VARCHAR(36)  NOT NULL,
    event_type      VARCHAR(20)  NOT NULL CHECK (event_type IN ('outage', 'offline')),
    started_at      TIMESTAMPTZ  NOT NULL,
    ended_at        TIMESTAMPTZ  NULL,
    corroborated    BOOLEAN      NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_conn_device ON connectivity_events (device_id);
CREATE INDEX IF NOT EXISTS idx_conn_office ON connectivity_events (office_id);

-- -------------------------------------------------------------------------
-- agent_lifecycle_events  — boot / shutdown markers (Task 7 / FR14)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS agent_lifecycle_events (
    id              BIGSERIAL    PRIMARY KEY,
    device_id       VARCHAR(36)  NOT NULL,
    event           VARCHAR(20)  NOT NULL CHECK (event IN ('BOOT', 'SHUTDOWN')),
    event_at        TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_lifecycle_device ON agent_lifecycle_events (device_id);
