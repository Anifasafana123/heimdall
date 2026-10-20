-- Migration 001 — Tables required by Safana's background processing tasks
-- Run with: sqlx migrate run

-- -------------------------------------------------------------------------
-- offices
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS offices (
    id                  VARCHAR(36)  NOT NULL PRIMARY KEY,  -- UUID
    name                VARCHAR(255) NOT NULL,
    daily_ceiling_bytes BIGINT       NOT NULL DEFAULT 524288000, -- 500 MB
    ceiling_hit         BOOLEAN      NOT NULL DEFAULT FALSE,
    ceiling_reset_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at          DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- devices
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS devices (
    device_id           VARCHAR(36)  NOT NULL PRIMARY KEY,  -- UUID / cert serial
    office_id           VARCHAR(36)  NOT NULL,
    is_active           BOOLEAN      NOT NULL DEFAULT TRUE,
    last_heartbeat_at   DATETIME     NULL,
    last_known_seq      BIGINT       NOT NULL DEFAULT 0,
    created_at          DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_device_office FOREIGN KEY (office_id) REFERENCES offices(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- pending_results  — ingest queue (Task 1)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS pending_results (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id       VARCHAR(36)     NOT NULL,
    seq             BIGINT          NOT NULL,
    payload         JSON            NOT NULL,
    received_at     DATETIME(3)     NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    INDEX idx_pending_device (device_id),
    INDEX idx_pending_received (received_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- results  — append-only; no UPDATE or DELETE granted to the app role
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS results (
    id                  BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id           VARCHAR(36)     NOT NULL,
    seq                 BIGINT          NOT NULL,
    payload             JSON            NOT NULL,
    payload_hash        VARCHAR(64)     NOT NULL,  -- SHA-256 hex
    latency_ms          DOUBLE          NULL,
    jitter_ms           DOUBLE          NULL,
    packet_loss_pct     DOUBLE          NULL,
    throughput_mbps     DOUBLE          NULL,
    link_speed_mbps     DOUBLE          NULL,
    bytes_used          BIGINT          NOT NULL DEFAULT 0,
    anomaly_checked     BOOLEAN         NOT NULL DEFAULT FALSE,
    received_at         DATETIME(3)     NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    UNIQUE KEY uq_device_seq (device_id, seq),
    INDEX idx_results_device    (device_id),
    INDEX idx_results_received  (received_at),
    INDEX idx_results_hash      (payload_hash)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- quarantine  — rows flagged by anomaly detector (Task 2)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS quarantine (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    result_id       BIGINT UNSIGNED NOT NULL,
    reason          VARCHAR(255)    NOT NULL,
    quarantined_at  DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reviewed        BOOLEAN         NOT NULL DEFAULT FALSE,
    reviewed_at     DATETIME        NULL,
    reviewed_by     VARCHAR(255)    NULL,   -- maintainer id
    CONSTRAINT fk_quarantine_result FOREIGN KEY (result_id) REFERENCES results(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- speedtest_leases  — one active lease per office (Task 3)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS speedtest_leases (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id       VARCHAR(36)     NOT NULL,
    status          ENUM('pending','active','expired','completed') NOT NULL DEFAULT 'pending',
    requested_at    DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    granted_at      DATETIME        NULL,
    expired_at      DATETIME        NULL,
    INDEX idx_lease_device (device_id),
    INDEX idx_lease_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- merkle_roots  — daily Merkle tree roots (Task 5)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS merkle_roots (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    tree_date       DATE            NOT NULL UNIQUE,
    root_hash       VARCHAR(64)     NOT NULL,  -- SHA-256 hex
    leaf_count      BIGINT UNSIGNED NOT NULL,
    built_at        DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- rollups_5min / rollups_1hr / rollups_1day  (Task 6)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS rollups_5min (
    id                      BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id               VARCHAR(36)     NOT NULL,
    bucket_start            DATETIME        NOT NULL,
    bucket_end              DATETIME        NOT NULL,
    median_latency_ms       DOUBLE          NULL,
    p10_latency_ms          DOUBLE          NULL,
    p90_latency_ms          DOUBLE          NULL,
    median_throughput_mbps  DOUBLE          NULL,
    p10_throughput_mbps     DOUBLE          NULL,
    p90_throughput_mbps     DOUBLE          NULL,
    median_jitter_ms        DOUBLE          NULL,
    packet_loss_pct         DOUBLE          NULL,
    sample_count            INT             NOT NULL DEFAULT 0,
    UNIQUE KEY uq_5min_device_bucket (device_id, bucket_start),
    INDEX idx_5min_bucket (bucket_start)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS rollups_1hr (
    id                      BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id               VARCHAR(36)     NOT NULL,
    bucket_start            DATETIME        NOT NULL,
    bucket_end              DATETIME        NOT NULL,
    median_latency_ms       DOUBLE          NULL,
    p10_latency_ms          DOUBLE          NULL,
    p90_latency_ms          DOUBLE          NULL,
    median_throughput_mbps  DOUBLE          NULL,
    p10_throughput_mbps     DOUBLE          NULL,
    p90_throughput_mbps     DOUBLE          NULL,
    median_jitter_ms        DOUBLE          NULL,
    packet_loss_pct         DOUBLE          NULL,
    sample_count            INT             NOT NULL DEFAULT 0,
    UNIQUE KEY uq_1hr_device_bucket (device_id, bucket_start),
    INDEX idx_1hr_bucket (bucket_start)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS rollups_1day (
    id                      BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id               VARCHAR(36)     NOT NULL,
    bucket_date             DATE            NOT NULL,
    median_latency_ms       DOUBLE          NULL,
    p10_latency_ms          DOUBLE          NULL,
    p90_latency_ms          DOUBLE          NULL,
    median_throughput_mbps  DOUBLE          NULL,
    p10_throughput_mbps     DOUBLE          NULL,
    p90_throughput_mbps     DOUBLE          NULL,
    median_jitter_ms        DOUBLE          NULL,
    packet_loss_pct         DOUBLE          NULL,
    sample_count            INT             NOT NULL DEFAULT 0,
    UNIQUE KEY uq_1day_device_bucket (device_id, bucket_date)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- agent_lifecycle_events  — SHUTDOWN / BOOT markers from agents (Task 7)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS agent_lifecycle_events (
    id          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id   VARCHAR(36)     NOT NULL,
    event       ENUM('BOOT','SHUTDOWN') NOT NULL,
    event_at    DATETIME(3)     NOT NULL,
    received_at DATETIME(3)     NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    INDEX idx_lifecycle_device (device_id),
    INDEX idx_lifecycle_event  (event_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- -------------------------------------------------------------------------
-- connectivity_events  — outage / offline timeline (Task 7)
-- -------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS connectivity_events (
    id              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    device_id       VARCHAR(36)     NOT NULL,
    office_id       VARCHAR(36)     NOT NULL,
    event_type      ENUM('outage','offline') NOT NULL,
    started_at      DATETIME(3)     NOT NULL,
    ended_at        DATETIME(3)     NULL,       -- NULL = still ongoing
    corroborated    BOOLEAN         NOT NULL DEFAULT FALSE,
    INDEX idx_event_device  (device_id),
    INDEX idx_event_office  (office_id),
    INDEX idx_event_started (started_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
