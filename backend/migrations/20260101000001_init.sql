-- Initial schema — PostgreSQL
-- Robeeta (issue #2). Full schema is Yaswini's (issue #3).

CREATE TABLE IF NOT EXISTS computers (
    id              UUID        NOT NULL DEFAULT gen_random_uuid(),
    hostname        TEXT        NOT NULL,
    mac_address     TEXT        NOT NULL,
    ip_address      TEXT        NOT NULL,
    owner           TEXT        NOT NULL,
    is_authorized   BOOLEAN     NOT NULL DEFAULT TRUE,
    registered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ,
    PRIMARY KEY (id),
    UNIQUE (mac_address)
);

CREATE TABLE IF NOT EXISTS metrics (
    id              BIGSERIAL   PRIMARY KEY,
    computer_id     UUID        NOT NULL REFERENCES computers(id) ON DELETE CASCADE,
    metric_kind     TEXT        NOT NULL,
    value           DOUBLE PRECISION NOT NULL,
    recorded_at     TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_metrics_computer_kind_time
    ON metrics (computer_id, metric_kind, recorded_at);

-- Approved metric kinds (enforced in application layer):
--   cpu_usage, memory_usage, disk_usage, network_rx, network_tx

CREATE TABLE IF NOT EXISTS admin_users (
    id              BIGSERIAL   PRIMARY KEY,
    username        TEXT        NOT NULL UNIQUE,
    password_hash   TEXT        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
