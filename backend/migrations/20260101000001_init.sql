-- Initial schema for the network monitoring system.
-- MySQL Community Edition. Using utf8mb4.

CREATE TABLE IF NOT EXISTS computers (
    id              CHAR(36)     NOT NULL,
    hostname        VARCHAR(255) NOT NULL,
    mac_address     VARCHAR(17)  NOT NULL,
    ip_address      VARCHAR(45)  NOT NULL,
    owner           VARCHAR(255) NOT NULL,
    is_authorized   TINYINT(1)   NOT NULL DEFAULT 1,
    registered_at   DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    last_seen_at    DATETIME(3)  NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uq_computers_mac (mac_address)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS metrics (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    computer_id     CHAR(36)     NOT NULL,
    metric_kind     VARCHAR(32)  NOT NULL,
    value           DOUBLE       NOT NULL,
    recorded_at     DATETIME(3)  NOT NULL,
    PRIMARY KEY (id),
    KEY idx_metrics_computer_kind_time (computer_id, metric_kind, recorded_at),
    CONSTRAINT fk_metrics_computer FOREIGN KEY (computer_id)
        REFERENCES computers (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- Approved metric kinds (enforced in application layer, documented here):
--   cpu_usage, memory_usage, disk_usage, network_rx, network_tx

CREATE TABLE IF NOT EXISTS admin_users (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    username        VARCHAR(64)  NOT NULL,
    password_hash   VARCHAR(255) NOT NULL,
    created_at      DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    UNIQUE KEY uq_admin_username (username)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
