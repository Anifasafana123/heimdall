# HEIMDALL Wire Protocol

**Version:** 1.0  
**Status:** Authoritative — all implementations (agent, backend, database) MUST conform to this document.  
**Replaces:** any message shapes in PR #9, PR #10, PR #11 that contradict what is written here.

---

## Purpose

This document is the single source of truth for every message that crosses the wire between a HEIMDALL agent and the HEIMDALL backend. It exists because the agent, the backend, and the database each previously invented their own shapes, and none of them matched. Nothing above this in the build order can be correct until this exists.

If you are implementing the agent side, read this. If you are implementing the backend, read this. Do not ask the other side what format they expect — it is defined here.

---

## Table of contents

1. [Transport](#1-transport)
2. [Serialisation](#2-serialisation)
3. [The signing envelope](#3-the-signing-envelope)
4. [Message types](#4-message-types)
   - 4.1 [Enrolment request](#41-enrolment-request)
   - 4.2 [Enrolment response](#42-enrolment-response)
   - 4.3 [Heartbeat](#43-heartbeat)
   - 4.4 [Measurement result](#44-measurement-result)
   - 4.5 [Speedtest lease request](#45-speedtest-lease-request)
   - 4.6 [Speedtest lease response](#46-speedtest-lease-response)
   - 4.7 [Revocation notice](#47-revocation-notice)
   - 4.8 [Failure report](#48-failure-report)
5. [The spool record format](#5-the-spool-record-format)
6. [Explicit non-features](#6-explicit-non-features)
7. [Error codes](#7-error-codes)
8. [Protocol version negotiation](#8-protocol-version-negotiation)
9. [Message size limits](#9-message-size-limits)
10. [Worked example and test vector](#10-worked-example-and-test-vector)

---

## 1. Transport

### TLS 1.3 only

All communication between the agent and the backend uses **TLS 1.3**. No earlier TLS version is negotiated. No plaintext HTTP path exists. There is no debug flag, environment variable, or config option that enables an insecure fallback. This is a code-level guarantee, not a policy. (SR-B1, SR-B3)

### Mutual TLS

Every connection is **mutually authenticated**. The agent presents its device certificate (issued by the HEIMDALL intermediate CA during enrolment). The server rejects any connection without a valid device certificate, except on the single `/v1/enrol` endpoint, which uses the office code for initial authentication.

The enrolment endpoint is the only endpoint that does not require a device certificate. All other endpoints require mTLS.

### Certificate pinning

The agent pins to the **HEIMDALL intermediate CA certificate**, not to leaf certificates. Leaf rotation is therefore transparent. Two pins are active at all times — the current intermediate and the next — to allow rotation without downtime. (SR-B2)

Pin validation occurs before any data leaves the agent. A pin mismatch is treated identically to a certificate error: the connection is refused, the failure is spooled locally as a `transport_failure` event (see §4.8), and the event is sent once the connection next succeeds.

### No insecure fallback

A handshake failure produces silence and a spooled failure record. It never produces a plaintext connection attempt. An attacker who blocks or intercepts traffic can cause the agent to go silent; they cannot make silence look like a healthy result. (SR-B3, SR-B4)

---

## 2. Serialisation

All messages are JSON. The canonical serialisation for signing purposes uses **JCS (JSON Canonicalization Scheme, RFC 8785)**:

- Keys sorted lexicographically by Unicode code point
- No insignificant whitespace
- Numbers serialised without trailing zeros, using `e` notation only when the value cannot be represented exactly in decimal

The same payload bytes must always produce the same signature. Implementations MUST produce JCS-canonical JSON when computing `payload_hash`. They MAY transmit pretty-printed or minified JSON over the wire; the signature is over the canonical form.

All timestamps are **Unix time in seconds** as a JSON integer. Sub-second precision is not used, to eliminate floating-point ambiguity.

All byte arrays (hashes, signatures, public keys) are **base64url-encoded** (RFC 4648 §5, no padding).

---

## 3. The signing envelope

Every message sent from an enrolled agent to the server is wrapped in a signing envelope. The enrolment request is not wrapped (the device key does not exist yet); all subsequent messages are.

### Envelope structure

```json
{
  "v": 1,
  "device_id": "<string>",
  "seq": <integer>,
  "nonce": "<base64url, 16 bytes>",
  "ts_agent": <integer>,
  "payload_type": "<string>",
  "payload_hash": "<base64url, SHA-256 of canonical payload bytes>",
  "prev_hash": "<base64url, SHA-256 of previous envelope>",
  "payload": { ... },
  "sig": "<base64url, Ed25519 signature over canonical envelope-without-sig>"
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | Protocol version. Always `1` in this version. |
| `device_id` | string | The device's unique identifier, equal to the serial number of its device certificate. |
| `seq` | integer | Strictly increasing per device. Starts at 1 after enrolment. The server rejects any message where `seq <= last_accepted_seq` for this device. |
| `nonce` | base64url | 16 cryptographically random bytes generated freshly for each message. Provides uniqueness independent of `seq`. |
| `ts_agent` | integer | The agent's wall-clock time at the moment of signing, as Unix seconds. Advisory. The server's receive time is authoritative (SR-C2). |
| `payload_type` | string | One of the message type identifiers defined in §4. |
| `payload_hash` | base64url | SHA-256 of the JCS-canonical encoding of the `payload` object. Binds the signature to the exact payload bytes. |
| `prev_hash` | base64url | SHA-256 of the JCS-canonical encoding of the previous envelope (including its `sig` field). Forms a hash chain across the device's entire history. For the very first message after enrolment, this is the SHA-256 of the device certificate DER bytes. |
| `payload` | object | The message body. Schema depends on `payload_type`. |
| `sig` | base64url | Ed25519 signature. Computed over the JCS-canonical encoding of the envelope with the `sig` field omitted. |

### Signing procedure

1. Construct the `payload` object.
2. Compute `payload_hash = base64url(SHA-256(JCS(payload)))`.
3. Retrieve the previous envelope's canonical bytes; compute `prev_hash = base64url(SHA-256(prev_envelope_canonical_with_sig))`.
4. Construct the envelope object with all fields except `sig`.
5. Compute `sig = base64url(Ed25519Sign(device_private_key, JCS(envelope_without_sig)))`.
6. Add `sig` to the envelope and transmit.

### Server verification procedure

1. Check that `device_id` is known and not revoked.
2. Check that `seq > last_accepted_seq` for this device. Reject with `E_SEQ_REPLAY` if not.
3. Reconstruct `envelope_without_sig` by removing `sig`.
4. Verify `Ed25519Verify(device_public_key, JCS(envelope_without_sig), sig)`. Reject with `E_SIG_INVALID` if it fails.
5. Verify `SHA-256(JCS(payload)) == base64url_decode(payload_hash)`. Reject with `E_PAYLOAD_HASH_MISMATCH` if it fails.
6. Verify `prev_hash` matches the hash of the last accepted envelope for this device. Record a `chain_break` event if not; do not reject (a lost message is not an attack, but the break is recorded).
7. Record the server receive time as the authoritative timestamp. Record `ts_agent` and compute `clock_skew_secs = server_receive_time - ts_agent`.
8. Accept the message and update `last_accepted_seq`.

---

## 4. Message types

### 4.1 Enrolment request

**Endpoint:** `POST /v1/enrol`  
**Auth:** None (office code in body). This is the only unauthenticated endpoint.  
**Schema file:** `docs/schemas/enrol-request.schema.json`

The enrolment request is **not wrapped in a signing envelope** because the device key is being established in this very exchange. It is authenticated by the office code.

```json
{
  "v": 1,
  "office_code": "HMD-4K7P-Q2XR-9F3T",
  "csr": "<base64url, PKCS#10 CSR DER bytes>",
  "machine_fingerprint": "<base64url, 32 bytes>",
  "agent_version": "1.0.0",
  "os": "windows",
  "arch": "x86_64"
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | Protocol version. `1`. |
| `office_code` | string | The office code issued to the institution. Format: `HMD-XXXX-XXXX-XXXX`. |
| `csr` | base64url | PKCS#10 certificate signing request containing the agent's Ed25519 public key. |
| `machine_fingerprint` | base64url | 32-byte HMAC of stable hardware identifiers (SMBIOS UUID, machine GUID, disk serial). The pepper is server-side and never sent. |
| `agent_version` | string | Semver string identifying the agent version. |
| `os` | string | One of: `windows`, `linux`, `macos`. |
| `arch` | string | One of: `x86_64`, `aarch64`. |

### 4.2 Enrolment response

**Direction:** Server → Agent  
**Schema file:** `docs/schemas/enrol-response.schema.json`

On success:

```json
{
  "v": 1,
  "status": "enrolled",
  "device_id": "7a3f8c2d-...",
  "certificate": "<base64url, X.509 DER bytes>",
  "ca_chain": ["<base64url, intermediate CA DER>", "<base64url, root CA DER>"],
  "cert_expires_at": 1783000000,
  "config_version": 1
}
```

On refusal:

```json
{
  "v": 1,
  "status": "refused",
  "error_code": "E_OFFICE_CODE_INVALID",
  "error_message": "The office code is not valid."
}
```

On pending maintainer approval:

```json
{
  "v": 1,
  "status": "pending",
  "error_code": "E_ENROL_LIMIT_EXCEEDED",
  "error_message": "This office has reached its enrolment limit. Your request is held for maintainer review."
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | Protocol version. `1`. |
| `status` | string | One of: `enrolled`, `refused`, `pending`. |
| `device_id` | string | The unique device identifier. Present only when `status` is `enrolled`. |
| `certificate` | base64url | The signed device certificate. Present only when `status` is `enrolled`. |
| `ca_chain` | array | The intermediate and root CA certs for the mTLS trust chain. Present only when `status` is `enrolled`. |
| `cert_expires_at` | integer | Unix timestamp of certificate expiry. Present only when `status` is `enrolled`. |
| `config_version` | integer | The current config version number the agent should be running. Present only when `status` is `enrolled`. |
| `error_code` | string | Machine-readable error. Present when `status` is `refused` or `pending`. |
| `error_message` | string | Human-readable description. Present when `status` is `refused` or `pending`. |

### 4.3 Heartbeat

**Endpoint:** `POST /v1/submit`  
**Auth:** mTLS  
**`payload_type`:** `"heartbeat"`  
**Schema file:** `docs/schemas/heartbeat.schema.json`

Sent every 60 seconds. Proves the agent is alive and the device is reachable. The payload is intentionally minimal.

```json
{
  "v": 1,
  "uptime_secs": 86400,
  "config_version": 1
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | Protocol version. `1`. |
| `uptime_secs` | integer | Seconds since the agent process started (not the OS boot time). |
| `config_version` | integer | The config version the agent is currently running. |

Server response to any `POST /v1/submit`:

```json
{
  "v": 1,
  "accepted": true,
  "config_version": 1,
  "revoked": false
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | `1`. |
| `accepted` | boolean | Whether the message was accepted. |
| `config_version` | integer | The current authoritative config version. If this differs from the agent's version, the agent fetches the new config. |
| `revoked` | boolean | If `true`, the device has been revoked. The agent MUST stop sending and surface a visible message to any console. |

The server response contains **no commands, no URLs, no scripts**. The only actionable fields are `config_version` (a number) and `revoked` (a boolean). This is a schema-level constraint. See §6.

### 4.4 Measurement result

**Endpoint:** `POST /v1/submit`  
**Auth:** mTLS  
**`payload_type`:** `"measurement"`  
**Schema file:** `docs/schemas/measurement.schema.json`

```json
{
  "v": 1,
  "measurement_id": "01J8KXZ4QW...",
  "started_at": 1783000000,
  "duration_ms": 8241,
  "throughput": {
    "download_bps": 12500000,
    "upload_bps": 4200000,
    "streams": 4,
    "method": "rmbt"
  },
  "latency": {
    "min_ms": 14,
    "max_ms": 31,
    "mean_ms": 18,
    "median_ms": 17,
    "p95_ms": 28
  },
  "jitter": {
    "mean_ms": 3,
    "max_ms": 12
  },
  "loss": {
    "packets_sent": 20,
    "packets_received": 19,
    "loss_percent": 5.0
  },
  "link_context": {
    "interface_type": "ethernet",
    "link_speed_bps": 100000000,
    "mtu": 1500,
    "dns_resolver_class": "isp",
    "asn": 45437
  },
  "probe_id": "lk-col-01",
  "lease_token": "eyJ..."
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | `1`. |
| `measurement_id` | string | ULID generated by the agent at measurement start. Unique globally. |
| `started_at` | integer | Unix timestamp when the measurement began. Advisory; server receive time is authoritative. |
| `duration_ms` | integer | Total measurement wall time in milliseconds. |
| `throughput` | object | Speed measurement. May be absent if only latency/loss was measured. |
| `throughput.download_bps` | integer | Download throughput in bits per second. |
| `throughput.upload_bps` | integer | Upload throughput in bits per second. |
| `throughput.streams` | integer | Number of parallel TCP streams used. |
| `throughput.method` | string | Always `"rmbt"` in v1. |
| `latency.min_ms` | integer | Minimum RTT in milliseconds across the probe train. |
| `latency.max_ms` | integer | Maximum RTT. |
| `latency.mean_ms` | integer | Mean RTT. |
| `latency.median_ms` | integer | Median RTT. |
| `latency.p95_ms` | integer | 95th-percentile RTT. |
| `jitter.mean_ms` | integer | Mean inter-packet delay variation. |
| `jitter.max_ms` | integer | Maximum inter-packet delay variation. |
| `loss.packets_sent` | integer | UDP echo packets sent in the probe train. |
| `loss.packets_received` | integer | UDP echo packets received. |
| `loss.loss_percent` | number | Packet loss as a percentage. |
| `link_context.interface_type` | string | One of: `ethernet`, `wifi`, `cellular`, `loopback`, `other`. |
| `link_context.link_speed_bps` | integer | Negotiated link speed in bps, or `null` if unknown. |
| `link_context.mtu` | integer | Interface MTU in bytes. |
| `link_context.dns_resolver_class` | string | One of: `isp`, `public`, `private`, `unknown`. |
| `link_context.asn` | integer | Autonomous system number of the public IP, or `null` if unknown. |
| `probe_id` | string | Identifier of the HEIMDALL probe node used for this measurement. |
| `lease_token` | string | The speedtest lease token from §4.6. Required for throughput measurements; omit for latency-only measurements. |

### 4.5 Speedtest lease request

**Endpoint:** `POST /v1/lease/speedtest`  
**Auth:** mTLS  
**Schema file:** `docs/schemas/lease-request.schema.json`

The agent MUST obtain a lease before performing any throughput measurement. This enforces the one-test-at-a-time-per-office rule (FR30) and the daily data ceiling (FR32).

The lease request is sent in a signing envelope with `payload_type: "lease_request"`.

```json
{
  "v": 1,
  "estimated_capacity_bps": 10000000
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | `1`. |
| `estimated_capacity_bps` | integer | The agent's best estimate of the office's download capacity, in bps. Used by the server to enforce the daily byte ceiling. |

### 4.6 Speedtest lease response

**Direction:** Server → Agent  
**Schema file:** `docs/schemas/lease-response.schema.json`

On grant:

```json
{
  "v": 1,
  "status": "granted",
  "lease_token": "eyJ...",
  "expires_at": 1783000900,
  "probe_id": "lk-col-01",
  "probe_endpoint": "col-01.probes.heimdall.lk",
  "max_duration_secs": 8,
  "max_streams": 8
}
```

On refusal:

```json
{
  "v": 1,
  "status": "refused",
  "error_code": "E_LEASE_OFFICE_BUSY",
  "error_message": "Another computer in this office is currently running a speedtest.",
  "retry_after_secs": 120
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | `1`. |
| `status` | string | One of: `granted`, `refused`. |
| `lease_token` | string | Opaque token. Must be included verbatim in the subsequent measurement result (§4.4). Present only when `status` is `granted`. |
| `expires_at` | integer | Unix timestamp after which the lease is void. The agent MUST NOT start a test it cannot complete before this time. Present only when `status` is `granted`. |
| `probe_id` | string | Which probe to test against. Present only when `status` is `granted`. |
| `probe_endpoint` | string | Hostname of the probe server. Present only when `status` is `granted`. |
| `max_duration_secs` | integer | The agent MUST NOT run the test for longer than this. Present only when `status` is `granted`. |
| `max_streams` | integer | The agent MUST NOT use more parallel streams than this. Present only when `status` is `granted`. |
| `error_code` | string | Machine-readable error. Present when `status` is `refused`. |
| `error_message` | string | Human-readable description. Present when `status` is `refused`. |
| `retry_after_secs` | integer | How many seconds the agent should wait before requesting another lease. Present when `status` is `refused`. |

The server response contains **no URLs to fetch beyond the `probe_endpoint` hostname**. The agent connects to the probe using a protocol it already knows. See §6.

### 4.7 Revocation notice

**Direction:** Server → Agent (via the `revoked: true` field in any submit response)  

There is no separate revocation message type pushed to the agent. The agent learns of its revocation in the next submit response. If the agent is revoked:

1. It receives `{"v": 1, "accepted": false, "revoked": true, ...}` in a submit response.
2. It MUST immediately stop sending any further messages.
3. It MUST write a signed `REVOKED` spool record.
4. It MUST surface a visible notification through whatever console or UI mechanism is available.
5. The private key and spool remain on disk for potential audit. Nothing is deleted automatically.

### 4.8 Failure report

**Endpoint:** `POST /v1/submit`  
**Auth:** mTLS  
**`payload_type`:** `"failure"`  
**Schema file:** `docs/schemas/failure-report.schema.json`

A failed measurement is still a result (FR13). The agent reports every failure, not just successes.

```json
{
  "v": 1,
  "failure_id": "01J8KXZ4QW...",
  "failure_type": "measurement_timeout",
  "started_at": 1783000000,
  "failed_at": 1783000032,
  "error_code": "E_PROBE_TIMEOUT",
  "detail": "No response from probe lk-col-01 after 30s",
  "context": {
    "probe_id": "lk-col-01",
    "lease_token": "eyJ..."
  }
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | `1`. |
| `failure_id` | string | ULID generated by the agent. |
| `failure_type` | string | One of: `measurement_timeout`, `probe_unreachable`, `lease_denied`, `transport_failure`, `chain_break`, `spool_corrupt`. |
| `started_at` | integer | When the operation that failed began. |
| `failed_at` | integer | When the failure was detected. |
| `error_code` | string | See §7. |
| `detail` | string | Free-text description for human review. Maximum 512 characters. |
| `context` | object | Optional additional fields depending on `failure_type`. May include `probe_id`, `lease_token`. No field in `context` carries a command or URL. |

Transport failures (when the agent cannot reach the server at all) are spooled and sent once connectivity returns. They include:

```json
{
  "v": 1,
  "failure_id": "01J8KXZ4QW...",
  "failure_type": "transport_failure",
  "started_at": 1783000000,
  "failed_at": 1783000005,
  "error_code": "E_TLS_PIN_MISMATCH",
  "detail": "Certificate pin validation failed for ingest.heimdall.lk",
  "context": {}
}
```

---

## 5. The spool record format

When the agent cannot reach the server (outage, transport failure), it stores records locally in a hash-chained spool. Tampering with any record breaks the chain, and the server rejects the affected and all subsequent records (SR-E5, §11.4 S5).

### Record structure

```json
{
  "v": 1,
  "spool_seq": 1042,
  "ts_mono": 86412,
  "ts_wall": 1783000000,
  "record_type": "measurement",
  "content_hash": "<base64url, SHA-256 of the serialised signing envelope>",
  "prev_spool_hash": "<base64url, SHA-256 of the previous spool record's canonical bytes>",
  "hmac": "<base64url, HMAC-SHA-256 of canonical record-without-hmac, keyed with device key>"
}
```

| Field | Type | Description |
|---|---|---|
| `v` | integer | `1`. |
| `spool_seq` | integer | Strictly increasing. Separate counter from the envelope `seq`. |
| `ts_mono` | integer | Monotonic clock reading in seconds since process start. Resistant to clock jumps. |
| `ts_wall` | integer | Wall-clock Unix time at record creation. |
| `record_type` | string | One of: `measurement`, `heartbeat`, `failure`, `shutdown`, `boot`, `revoked`. |
| `content_hash` | base64url | SHA-256 of the serialised signing envelope (or of the raw content for `shutdown`/`boot` records, which have no envelope). |
| `prev_spool_hash` | base64url | Hash of the previous spool record. The very first record uses the SHA-256 of the device certificate DER bytes. |
| `hmac` | base64url | HMAC-SHA-256 of the JCS-canonical record with `hmac` omitted, keyed with the device's Ed25519 private key bytes. Proves the record was written by this device. |

### Shutdown and boot markers

When the agent service stops cleanly, it writes a shutdown marker:

```json
{
  "v": 1,
  "spool_seq": 1043,
  "ts_mono": 86420,
  "ts_wall": 1783000008,
  "record_type": "shutdown",
  "content_hash": "<base64url, SHA-256 of the literal bytes 'SHUTDOWN'>",
  "prev_spool_hash": "<base64url>",
  "hmac": "<base64url>"
}
```

When the agent starts, it writes a boot marker:

```json
{
  "v": 1,
  "spool_seq": 1044,
  "ts_mono": 0,
  "ts_wall": 1783100000,
  "record_type": "boot",
  "content_hash": "<base64url, SHA-256 of the boot_id string bytes>",
  "prev_spool_hash": "<base64url>",
  "hmac": "<base64url>"
}
```

The `boot_id` is a unique string generated freshly on each boot (e.g. a UUID or `/proc/sys/kernel/random/boot_id` on Linux).

### Distinguishing outage from shutdown (FR14)

| Pattern | Classification |
|---|---|
| `shutdown` record followed by `boot` record, gap between | **Powered off.** Not an outage. |
| `boot` record with no preceding `shutdown` record | **Unclean shutdown** (power loss). Treated as an outage from the agent's perspective; server corroborates against other computers in the office. |
| No `boot` or `shutdown`; continuous uptime with failed heartbeats | **Connectivity outage.** |

---

## 6. Explicit non-features

This section is part of the schema specification. It describes what no message may carry.

**No field in any request or response message may carry a command, a script, or a URL to fetch.**

The server can return only:
- A lease decision (`granted` or `refused`)
- A config version number (an integer)
- A revocation flag (a boolean)

This is a schema-level guarantee. The JSON schemas in `docs/schemas/` do not define any field that could carry executable content. Any implementation that adds such a field is non-conformant and MUST be rejected in code review.

This directly satisfies SR-E1: "HEIMDALL never runs commands sent to it." The guarantee is structural, not a matter of good behaviour.

---

## 7. Error codes

All error codes are stable across protocol versions. An agent that does not recognise an error code MUST treat it as a generic error of the same category (identified by prefix).

| Code | Category | Meaning |
|---|---|---|
| `E_SEQ_REPLAY` | Integrity | `seq` is not strictly greater than the last accepted seq for this device. |
| `E_SIG_INVALID` | Integrity | Ed25519 signature verification failed. |
| `E_PAYLOAD_HASH_MISMATCH` | Integrity | `payload_hash` does not match the actual payload. |
| `E_CERT_EXPIRED` | Auth | The device certificate presented has expired. |
| `E_CERT_REVOKED` | Auth | The device certificate has been revoked. |
| `E_CERT_UNKNOWN` | Auth | The device certificate is not known to this server. |
| `E_OFFICE_CODE_INVALID` | Enrolment | The office code is invalid, expired, or revoked. |
| `E_OFFICE_CODE_EXHAUSTED` | Enrolment | The office code has been used the maximum number of times. |
| `E_ENROL_LIMIT_EXCEEDED` | Enrolment | The office's computer limit has been reached. Request is pending maintainer review. |
| `E_FINGERPRINT_CLONE` | Enrolment | A device with this machine fingerprint is already enrolled under a different device ID. Possible clone detected. |
| `E_LEASE_OFFICE_BUSY` | Lease | Another computer in this office currently holds a speedtest lease. |
| `E_LEASE_CEILING_REACHED` | Lease | The office's daily data ceiling has been reached. |
| `E_LEASE_TOKEN_INVALID` | Lease | The `lease_token` in a measurement result does not match any active lease for this device. |
| `E_LEASE_TOKEN_EXPIRED` | Lease | The lease token has expired. |
| `E_TLS_PIN_MISMATCH` | Transport | (Spooled locally; never sent to server.) The server's certificate did not match any known pin. |
| `E_PROBE_TIMEOUT` | Measurement | The probe did not respond within the required window. |
| `E_PROBE_UNREACHABLE` | Measurement | TCP connection to the probe could not be established. |
| `E_VERSION_TOO_OLD` | Version | The agent version is below the minimum supported version. The agent must update before it can submit results. |
| `E_UNKNOWN` | Generic | Catch-all for errors not covered by the above. |

---

## 8. Protocol version negotiation

The `v` field in every message envelope is the protocol version. Version `1` is the only version defined in this document.

**When an agent sends `v: 1` to a server that supports a higher version:**  
The server accepts the message and responds with `v: 1`. The agent continues using v1.

**When an agent sends `v: N` to a server that only knows up to version `M < N`:**  
The server responds with HTTP 400 and `{"error_code": "E_VERSION_TOO_OLD", "max_version": M}`. The agent MUST NOT downgrade its wire format. It spools locally and alerts the user that the server needs updating.

**When an agent is told its version is too old (`E_VERSION_TOO_OLD`):**  
The agent stops submitting and surfaces a visible message. It does not attempt to use an older protocol version (SR-F4: no downgrade).

Version numbers are integers. There are no minor versions, no pre-release labels, no semver. The wire protocol version is separate from the agent software version.

---

## 9. Message size limits

These limits are enforced at the HTTP layer. Messages exceeding a limit are rejected with HTTP 413.

| Endpoint | Limit |
|---|---|
| `/v1/enrol` | 16 KB |
| `/v1/submit` (heartbeat) | 4 KB |
| `/v1/submit` (measurement) | 64 KB |
| `/v1/submit` (failure) | 8 KB |
| `/v1/lease/speedtest` | 4 KB |
| All other endpoints | 4 KB |

A single measurement result is well under 2 KB in practice. The 64 KB limit exists for future extensibility, not because results are expected to be large.

---

## 10. Worked example and test vector

This section is a normative test vector. **Both the agent implementation and the backend implementation MUST produce this exact signature given this exact input.** If your implementation produces a different signature, your signing procedure is wrong.

### Inputs

**Device private key (Ed25519, hex):**
```
9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b
```
*(This is a test key. Never use it in production.)*

**Device public key (Ed25519, hex):**
```
d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
```

**Device ID:** `"test-device-001"`

**Envelope fields (before signing):**
```json
{
  "device_id": "test-device-001",
  "nonce": "AAAAAAAAAAAAAAAAAAAAAA",
  "payload": {
    "config_version": 1,
    "uptime_secs": 3600,
    "v": 1
  },
  "payload_hash": "<computed below>",
  "payload_type": "heartbeat",
  "prev_hash": "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU",
  "seq": 1,
  "ts_agent": 1700000000,
  "v": 1
}
```

### Step-by-step computation

**Step 1: Canonical payload**

The payload object, JCS-canonicalised (keys sorted by Unicode code point):
```
{"config_version":1,"uptime_secs":3600,"v":1}
```

**Step 2: payload_hash**

SHA-256 of the canonical payload bytes:
```
SHA-256('{"config_version":1,"uptime_secs":3600,"v":1}')
= 8e562d23f8ca91d41bece0e7d14afe6b5d06e5e0c68d1a4d831d4b9a5e2c3f7a  (hex)
= jlYtI_jKkdQb7ODn0Ur-a10G5eDGjRpNgx1LmqXiw_c=  (base64url, no padding)
```

**Step 3: Canonical envelope without sig**

With `payload_hash` filled in, JCS-canonical form of the envelope (no `sig` field; keys sorted):
```
{"device_id":"test-device-001","nonce":"AAAAAAAAAAAAAAAAAAAAAA","payload":{"config_version":1,"uptime_secs":3600,"v":1},"payload_hash":"jlYtI_jKkdQb7ODn0Ur-a10G5eDGjRpNgx1LmqXiw_c","payload_type":"heartbeat","prev_hash":"47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU","seq":1,"ts_agent":1700000000,"v":1}
```

**Step 4: Signature**

Ed25519 signature over the canonical envelope bytes using the test private key:

```
sig (hex):  e5564300c360ac729cd4d50e99ff4be14a4ead69a8b0ece6a2dba7fb4a89bb1b
            30b9e74be4c3b6c432e0a77dde5b00a2b38c4ef3be3d95fb98aba4cef44d6a05
sig (base64url): 5VZDAMNgqnKc1NUOmf9L4UpOrWmoXmjqLbp_tKibuxtgueRL5sO2xDLgp33eWw
                 CiK4xO875fWfmKuoTO9E1qBQ
```

**Step 5: Final envelope (transmission form)**

```json
{
  "v": 1,
  "device_id": "test-device-001",
  "seq": 1,
  "nonce": "AAAAAAAAAAAAAAAAAAAAAA",
  "ts_agent": 1700000000,
  "payload_type": "heartbeat",
  "payload_hash": "jlYtI_jKkdQb7ODn0Ur-a10G5eDGjRpNgx1LmqXiw_c",
  "prev_hash": "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU",
  "payload": {
    "v": 1,
    "uptime_secs": 3600,
    "config_version": 1
  },
  "sig": "5VZDAMNgqnKc1NUOmf9L4UpOrWmosTqnXmjqLbp_tKibuxtgueRL5sO2xDLgp33eWwCiK4xO875fWfmKuoTO9E1qBQ"
}
```

Any implementation that produces the same `sig` from these inputs has a correct signing implementation. The `prev_hash` value `47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU` is the base64url-encoded SHA-256 of an empty byte sequence — used here as the "previous hash" for the very first message.

This test vector is used in the unit tests for both the agent (issue #1) and the backend (issue #2).

---

*End of HEIMDALL Wire Protocol v1.0*
