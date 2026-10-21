-- =============================================================================
-- HEIMDALL — Initial Schema
-- Migration: 001_initial_schema.sql
-- Database:  heimdall (PostgreSQL 17)
-- Author:    Yaswini (Issue #3)
-- Protocol:  docs/PROTOCOL.md v1
-- =============================================================================
-- Design rules
--   * All primary keys are BIGSERIAL (auto-increment BIGINT).
--   * All foreign keys are named fk_<child_table>_<parent_table>.
--   * Timestamps use TIMESTAMPTZ (UTC, microsecond precision).
--   * The metrics table is INSERT-only — no UPDATE or DELETE ever issued
--     by the application. Archive is append-only (SR-C4, PRD §14).
--   * Soft deletes: deleted_at TIMESTAMPTZ NULL on mutable tables.
--   * Envelope fields (device_id, seq, nonce, ts_agent, payload_hash,
--     prev_hash, sig) match docs/PROTOCOL.md §3 exactly.
-- =============================================================================


-- -----------------------------------------------------------------------------
-- 1. OFFICES
--    One row per registered government institution.
-- -----------------------------------------------------------------------------
CREATE TABLE offices (
    id                BIGSERIAL       PRIMARY KEY,
    name              VARCHAR(200)    NOT NULL,
    province          VARCHAR(100)    NOT NULL DEFAULT '',
    district          VARCHAR(100)    NOT NULL DEFAULT '',

    -- Argon2id hash of the enrolment ticket — never plaintext
    office_code_hash  VARCHAR(255)    NOT NULL,

    -- Maximum computers this office may enrol (FR5)
    max_computers     SMALLINT        NOT NULL DEFAULT 1 CHECK (max_computers >= 1),

    is_active         BOOLEAN         NOT NULL DEFAULT TRUE,
    code_revoked_at   TIMESTAMPTZ     NULL,
    code_expires_at   TIMESTAMPTZ     NULL,

    created_at        TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_at        TIMESTAMPTZ     NULL,

    CONSTRAINT uq_offices_name UNIQUE (name)
);


-- -----------------------------------------------------------------------------
-- 2. COMPUTERS
--    One row per enrolled computer.
--    device_id = certificate serial number (PROTOCOL.md §3, §4.2).
-- -----------------------------------------------------------------------------
CREATE TABLE computers (
    id                      BIGSERIAL       PRIMARY KEY,
    office_id               BIGINT          NOT NULL
                                REFERENCES offices (id),

    -- Certificate serial number — used as device_id in the signing envelope
    device_id               VARCHAR(128)    NOT NULL,

    -- Anonymised public label e.g. "Computer 3", randomly assigned (SR-H1)
    display_label           VARCHAR(20)     NOT NULL,

    -- SHA-256 hex of the Ed25519 public key — clone detection (SR-A2)
    pubkey_fingerprint      CHAR(64)        NOT NULL,

    -- HMAC of stable hardware IDs — re-enrolment detection (SR-A6)
    machine_fingerprint_hash VARCHAR(255)   NOT NULL,

    -- Device certificate details
    cert_serial             VARCHAR(64)     NOT NULL DEFAULT '',
    cert_expires_at         TIMESTAMPTZ     NULL,

    -- Last accepted seq for this device — server enforces seq > this (SR-C2)
    last_accepted_seq       BIGINT          NOT NULL DEFAULT 0,

    -- Enrolment lifecycle
    enrolled_at             TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    pending_approval_at     TIMESTAMPTZ     NULL,  -- NULL = approved
    reviewed_by             BIGINT          NULL,  -- FK added after users table
    reviewed_at             TIMESTAMPTZ     NULL,

    -- Revocation (SR-A5)
    revoked_at              TIMESTAMPTZ     NULL,
    revoked_by              BIGINT          NULL,  -- FK added after users table
    revocation_reason       VARCHAR(200)    NULL,

    deleted_at              TIMESTAMPTZ     NULL,
    created_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_computers_device_id   UNIQUE (device_id),
    CONSTRAINT uq_computers_cert_serial UNIQUE (cert_serial)
);

CREATE INDEX idx_computers_office_id   ON computers (office_id);
CREATE INDEX idx_computers_pubkey_fp   ON computers (pubkey_fingerprint);
CREATE INDEX idx_computers_pending     ON computers (pending_approval_at, revoked_at, deleted_at)
    WHERE pending_approval_at IS NOT NULL;
CREATE INDEX idx_computers_office_active ON computers (office_id, revoked_at, deleted_at)
    WHERE revoked_at IS NULL AND deleted_at IS NULL;


-- -----------------------------------------------------------------------------
-- 3. METRICS
--    Append-only time-series table. One row per signed envelope received.
--    Application role: INSERT + SELECT only — no UPDATE, no DELETE (SR-C4).
--
--    Envelope fields match PROTOCOL.md §3 exactly:
--      device_id, seq, nonce, ts_agent, payload_hash, prev_hash, sig
--
--    payload_type values match PROTOCOL.md §4:
--      heartbeat | measurement | failure | lease_request
--
--    quality values:
--      ok          — passed validation, in public record
--      quarantine  — held for maintainer review (SR-C3)
--      approved    — maintainer approved a quarantined row
--      rejected    — maintainer rejected, excluded from public view
-- -----------------------------------------------------------------------------
CREATE TABLE metrics (
    id              BIGSERIAL       PRIMARY KEY,
    computer_id     BIGINT          NOT NULL REFERENCES computers (id),
    office_id       BIGINT          NOT NULL REFERENCES offices (id),

    -- ── Signing envelope fields (PROTOCOL.md §3) ──────────────────────────
    -- Device identifier = certificate serial (PROTOCOL.md §3)
    device_id       VARCHAR(128)    NOT NULL,
    -- Strictly increasing per device; UNIQUE constraint blocks replays (SR-C2)
    seq             BIGINT          NOT NULL CHECK (seq >= 1),
    -- Base64url 16-byte random nonce, 22 chars (PROTOCOL.md §3)
    nonce           CHAR(22)        NOT NULL,
    -- Agent wall-clock Unix seconds (advisory — SR-C2)
    ts_agent        BIGINT          NOT NULL,
    -- Difference: server_receive_time − ts_agent, seconds
    clock_skew_secs INTEGER         NULL,
    -- payload_type from PROTOCOL.md §4
    payload_type    VARCHAR(20)     NOT NULL
                        CHECK (payload_type IN ('heartbeat','measurement','failure','lease_request')),
    -- Base64url SHA-256 of canonical payload, 43 chars (PROTOCOL.md §3)
    payload_hash    CHAR(43)        NOT NULL,
    -- Base64url SHA-256 of previous envelope, 43 chars (PROTOCOL.md §3)
    prev_hash       CHAR(43)        NOT NULL,
    -- Base64url Ed25519 signature, 86 chars (PROTOCOL.md §3)
    sig             CHAR(86)        NOT NULL,

    -- ── Server timestamps ─────────────────────────────────────────────────
    -- Server receive time — authoritative for the archive (SR-C2)
    recorded_at     TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    -- ── Measurement payload fields ────────────────────────────────────────
    -- Latency (measurement payload — PROTOCOL.md §4.4)
    latency_min_ms      INTEGER     NULL,
    latency_max_ms      INTEGER     NULL,
    latency_mean_ms     INTEGER     NULL,
    latency_median_ms   INTEGER     NULL,
    latency_p95_ms      INTEGER     NULL,

    -- Jitter
    jitter_mean_ms      INTEGER     NULL,
    jitter_max_ms       INTEGER     NULL,

    -- Packet loss
    packets_sent        INTEGER     NULL,
    packets_received    INTEGER     NULL,
    loss_percent        DOUBLE PRECISION NULL,

    -- Throughput (present only when throughput object is in the payload)
    download_bps        BIGINT      NULL,
    upload_bps          BIGINT      NULL,
    throughput_streams  SMALLINT    NULL,

    -- Link context
    interface_type      VARCHAR(20) NULL
                            CHECK (interface_type IN ('ethernet','wifi','cellular','loopback','other')),
    link_speed_bps      BIGINT      NULL,
    mtu                 SMALLINT    NULL,
    dns_resolver_class  VARCHAR(10) NULL
                            CHECK (dns_resolver_class IN ('isp','public','private','unknown')),
    -- ASN only — no IP address ever stored (PR4)
    asn                 INTEGER     NULL,

    -- Probe used for this measurement
    probe_id            VARCHAR(64) NULL,

    -- Heartbeat fields
    uptime_secs         BIGINT      NULL,

    -- ── Quality / validation ──────────────────────────────────────────────
    quality             VARCHAR(12) NOT NULL DEFAULT 'ok'
                            CHECK (quality IN ('ok','quarantine','approved','rejected')),
    quarantine_reason   VARCHAR(200) NULL,
    reviewed_by         BIGINT      NULL,  -- FK added after users table
    reviewed_at         TIMESTAMPTZ NULL,

    CONSTRAINT uq_metrics_device_seq UNIQUE (computer_id, seq)
);

-- Time-range indexes (PROTOCOL.md drives which queries the dashboard needs)
CREATE INDEX idx_metrics_computer_time
    ON metrics (computer_id, recorded_at DESC);

CREATE INDEX idx_metrics_office_time
    ON metrics (office_id, recorded_at DESC);

-- Fetch latest per computer + payload_type
CREATE INDEX idx_metrics_latest_per_computer
    ON metrics (computer_id, payload_type, quality, recorded_at DESC);

-- History range for a computer
CREATE INDEX idx_metrics_history_range
    ON metrics (computer_id, payload_type, recorded_at, quality);

-- Office dashboard summary
CREATE INDEX idx_metrics_office_summary
    ON metrics (office_id, quality, recorded_at, payload_type);

-- Public map live pin — last heartbeat per office
CREATE INDEX idx_metrics_office_last_seen
    ON metrics (quality, payload_type, office_id, recorded_at DESC);

-- Outage history
CREATE INDEX idx_metrics_outages
    ON metrics (office_id, payload_type, recorded_at, computer_id)
    WHERE payload_type IN ('failure');

-- Merkle proof lookup
CREATE INDEX idx_metrics_payload_hash
    ON metrics (payload_hash);


-- -----------------------------------------------------------------------------
-- 4. ALERTS
--    System-generated anomaly and security alerts.
-- -----------------------------------------------------------------------------
CREATE TABLE alerts (
    id              BIGSERIAL       PRIMARY KEY,
    office_id       BIGINT          NULL REFERENCES offices (id),
    computer_id     BIGINT          NULL REFERENCES computers (id),
    metric_id       BIGINT          NULL REFERENCES metrics (id),

    alert_type      VARCHAR(60)     NOT NULL,
    severity        VARCHAR(10)     NOT NULL DEFAULT 'warning'
                        CHECK (severity IN ('info','warning','critical')),
    description     VARCHAR(500)    NOT NULL DEFAULT '',

    status          VARCHAR(20)     NOT NULL DEFAULT 'open'
                        CHECK (status IN ('open','ack','resolved','false_positive')),
    resolved_by     BIGINT          NULL,  -- FK added after users table
    resolved_at     TIMESTAMPTZ     NULL,
    resolution_note VARCHAR(500)    NULL,

    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alerts_office      ON alerts (office_id);
CREATE INDEX idx_alerts_computer    ON alerts (computer_id);
CREATE INDEX idx_alerts_type_status ON alerts (alert_type, status);
CREATE INDEX idx_alerts_created     ON alerts (created_at DESC);
CREATE INDEX idx_alerts_office_open ON alerts (office_id, status, created_at DESC)
    WHERE status = 'open';


-- -----------------------------------------------------------------------------
-- 5. USERS  (maintainer accounts)
-- -----------------------------------------------------------------------------
CREATE TABLE users (
    id              BIGSERIAL       PRIMARY KEY,
    username        VARCHAR(60)     NOT NULL,
    email           VARCHAR(254)    NOT NULL,
    password_hash   VARCHAR(255)    NOT NULL,

    -- Additive role flags as an array (SR-G2)
    roles           TEXT[]          NOT NULL DEFAULT ARRAY['auditor'],

    is_active       BOOLEAN         NOT NULL DEFAULT TRUE,
    last_login_at   TIMESTAMPTZ     NULL,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ     NULL,

    CONSTRAINT uq_users_username UNIQUE (username),
    CONSTRAINT uq_users_email    UNIQUE (email)
);

-- Add FK references that depend on users
ALTER TABLE computers
    ADD CONSTRAINT fk_computers_reviewed_by
        FOREIGN KEY (reviewed_by) REFERENCES users (id),
    ADD CONSTRAINT fk_computers_revoked_by
        FOREIGN KEY (revoked_by)  REFERENCES users (id);

ALTER TABLE metrics
    ADD CONSTRAINT fk_metrics_reviewed_by
        FOREIGN KEY (reviewed_by) REFERENCES users (id);

ALTER TABLE alerts
    ADD CONSTRAINT fk_alerts_resolved_by
        FOREIGN KEY (resolved_by) REFERENCES users (id);


-- -----------------------------------------------------------------------------
-- 6. WEBAUTHN_CREDENTIALS
--    WebAuthn hardware key registrations per maintainer (SR-G1).
-- -----------------------------------------------------------------------------
CREATE TABLE webauthn_credentials (
    id              BIGSERIAL       PRIMARY KEY,
    user_id         BIGINT          NOT NULL REFERENCES users (id),
    credential_id   VARCHAR(1024)   NOT NULL,
    public_key      TEXT            NOT NULL,
    aaguid          CHAR(36)        NOT NULL DEFAULT '',
    sign_count      BIGINT          NOT NULL DEFAULT 0,
    friendly_name   VARCHAR(100)    NOT NULL DEFAULT 'Security Key',
    registered_at   TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    last_used_at    TIMESTAMPTZ     NULL
);

CREATE INDEX idx_webauthn_user ON webauthn_credentials (user_id);


-- -----------------------------------------------------------------------------
-- 7. AUDIT_LOG
--    Append-only, hash-chained. Application role: INSERT + SELECT only (SR-G3).
-- -----------------------------------------------------------------------------
CREATE TABLE audit_log (
    id          BIGSERIAL       PRIMARY KEY,
    user_id     BIGINT          NULL REFERENCES users (id),
    ip_hash     VARCHAR(64)     NULL,  -- SHA-256 of IP, never raw (PR4)
    action      VARCHAR(80)     NOT NULL,
    target_type VARCHAR(40)     NULL,
    target_id   BIGINT          NULL,
    details     JSONB           NULL,
    prev_hash   CHAR(64)        NULL,  -- NULL for row 1
    row_hash    CHAR(64)        NOT NULL,
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_user      ON audit_log (user_id);
CREATE INDEX idx_audit_action    ON audit_log (action);
CREATE INDEX idx_audit_target    ON audit_log (target_type, target_id);
CREATE INDEX idx_audit_created   ON audit_log (created_at DESC);
CREATE INDEX idx_audit_user_time ON audit_log (user_id, created_at DESC);


-- -----------------------------------------------------------------------------
-- 8. CORRECTIONS
--    Corrections are new rows referencing the original — the original stays.
--    The archive is append-only; past results are never silently changed (SR-C4).
-- -----------------------------------------------------------------------------
CREATE TABLE corrections (
    id              BIGSERIAL       PRIMARY KEY,
    original_id     BIGINT          NOT NULL REFERENCES metrics (id),
    corrected_by    BIGINT          NOT NULL REFERENCES users (id),
    corrected_at    TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    reason          VARCHAR(500)    NOT NULL,
    -- The correction details as a JSON patch (what changed and why)
    detail          JSONB           NOT NULL
);

CREATE INDEX idx_corrections_original ON corrections (original_id);
