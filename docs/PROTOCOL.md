# HEIMDALL — Protocol Specification

**Version:** 1  
**Status:** draft — pending maintainer approval  
**Blocks:** issues #1, #2, #3, #5 and the dashboard  
**Refs:** HEIMDALL-PRD.md §11 (security), SR-A1–A6, SR-B1–B4, SR-C1–C2, SR-E1–E6, FR13, FR14, FR28–FR32

> This document defines every message the agent sends and the server returns.
> An implementer of either side must be able to build against this document
> alone, without asking the other side questions. See `docs/test_vectors.md`
> for concrete signing examples.

---

## 1. Transport

### 1.1 TLS

- **TLS 1.3 only.** No TLS 1.2, no plaintext, no `skip-verify` flag, not even
  behind a debug build tag or a config option (SR-B3).
- **Mutual TLS.** Every connection from the agent uses the device certificate
  issued at enrolment. The server rejects any connection without a valid client
  certificate.
- **No insecure fallback exists in the code.** A handshake failure is spooled
  locally as a `failure_report` with `reason: "tls_handshake_failed"` and
  reported once connectivity returns (SR-B3, SR-B4).

### 1.2 CA pinning (SR-B2)

The agent pins the **HEIMDALL intermediate CA**, not the leaf certificate
(so leaf rotation is free). Two pins are live at all times — `ca_pin_current`
and `ca_pin_next` — delivered in the `EnrolmentResponse`. The agent refuses
any server certificate that does not chain to one of these two pins.

### 1.3 Endpoints

| Path | Direction | Auth |
|---|---|---|
| `POST /enrol` | agent → server | office code in body |
| `POST /ingest` | agent → server | mTLS device cert |
| `POST /lease/speedtest` | agent → server | mTLS device cert |
| `GET /config` | agent → server | mTLS device cert |

All paths are under `https://ingest.heimdall.lk/v1/`.

### 1.4 Message size limits

| Message type | Max body size |
|---|---|
| EnrolmentRequest | 4 KB |
| SignedEnvelope (heartbeat / lifecycle) | 4 KB |
| SignedEnvelope (measurement result) | 16 KB |
| SignedEnvelope (failure report) | 4 KB |
| SpeedtestLeaseRequest | 2 KB |

The server returns `413 Content Too Large` for bodies that exceed these limits.

---

## 2. Protocol version negotiation

Every message carries `"protocol_version": 1`.

- If the server supports the version: process normally.
- If the server does **not** support the version: return
  `error_code: "UNSUPPORTED_VERSION"` with HTTP 400. The agent spools the
  message and waits for a software update; it does not retry indefinitely.

When the protocol is revised, the version number increments. Old agents that
send version 1 to a server that has moved to version 2 receive
`UNSUPPORTED_VERSION` and are expected to update via the TUF channel (SR-F3).

---

## 3. Enrolment

Enrolment is the only step that uses an office code. After enrolment, the
device certificate is the sole credential (SR-A1).

### 3.1 Request

`POST /v1/enrol`  
Body: `EnrolmentRequest` (see `schemas/enrolment_request.jsonschema`)

```json
{
  "protocol_version": 1,
  "office_code": "HMD-4K7P-Q2XR-9F3T",
  "device_fingerprint": "<64-hex HMAC-SHA-256 of hardware IDs>",
  "public_key_ed25519": "<base64 Ed25519 public key>",
  "timestamp": "2027-03-01T10:00:00Z"
}
```

**Office code rules (SR-A3):**
- Format: `HMD-` followed by three groups of four Crockford base32 characters.
- Stored Argon2id-hashed on the server. Never logged in plaintext.
- 5 wrong attempts per source /24 per hour → exponential backoff block.
- Enrolment beyond `max_enrolments` → held for maintainer approval, not
  silently refused (SR-A4).

**Device fingerprint:**  
`HMAC-SHA-256(server_pepper, SMBIOS_UUID || machine_GUID || disk_serial)`  
The pepper is server-side only. The raw hardware IDs never leave the device.
The same fingerprint re-enrolling → re-enrolment recorded, not a new device
(SR-A6). The same fingerprint on two concurrent device IDs → clone detected,
both refused (SR-A2).

### 3.2 Response

HTTP 201 on success.  
Body: `EnrolmentResponse` (see `schemas/enrolment_response.jsonschema`)

```json
{
  "protocol_version": 1,
  "device_id": "a1b2c3d4-0000-0000-0000-000000000001",
  "certificate_pem": "-----BEGIN CERTIFICATE-----\n...",
  "ca_pin_current": "<base64 SHA-256 of intermediate CA public key>",
  "ca_pin_next": "<base64 SHA-256 of next intermediate CA key>",
  "issued_at": "2027-03-01T10:00:01Z",
  "expires_at": "2027-05-30T10:00:01Z"
}
```

The device certificate is valid for 90 days and auto-renewed over mTLS before
expiry. The `device_id` is stable for the lifetime of the device registration.

### 3.3 Error responses

| HTTP | `error_code` | Meaning |
|---|---|---|
| 400 | `INVALID_OFFICE_CODE` | Malformed, expired or revoked code |
| 400 | `UNSUPPORTED_VERSION` | Protocol version not supported |
| 409 | `CLONE_DETECTED` | Same fingerprint already active on another device |
| 429 | `RATE_LIMITED` | Too many wrong-code attempts |
| 202 | `HELD_FOR_APPROVAL` | Enrolment beyond office limit; maintainer must approve |

---

## 4. The signed envelope

Every agent-to-server message after enrolment is wrapped in a `SignedEnvelope`
(see `schemas/envelope.jsonschema`). The payload is signed **before it touches
disk** (SR-C1).

### 4.1 Signing procedure

1. Serialise the `payload` object with **JCS** (RFC 8785): keys sorted
   lexicographically, no extra whitespace.
2. Compute `payload_hash = SHA-256(JCS(payload))`, hex-encoded.
3. Build the envelope object with all fields **except** `signature`.
4. Serialise the envelope object with JCS.
5. `signature = Ed25519Sign(device_private_key, JCS(envelope_without_signature))`
6. Base64-encode the signature and add it to the envelope.

The same payload must always produce the same bytes (canonical serialisation).
See `docs/test_vectors.md` for a worked example.

### 4.2 Envelope fields

| Field | Type | Description |
|---|---|---|
| `protocol_version` | integer | Always 1 |
| `device_id` | UUID string | Assigned at enrolment |
| `seq` | integer ≥ 1 | Strictly increasing per device. Server rejects `seq <= last_accepted` (SR-C2) |
| `nonce` | 32-hex string | 16 random bytes. Combined with `seq` against replay |
| `ts_agent` | RFC 3339 | Agent wall clock. Advisory; server receive time is authoritative (SR-C2) |
| `payload_hash` | 64-hex string | SHA-256 of JCS(payload) |
| `prev_hash` | 64-hex string | `payload_hash` of the previous envelope from this device. `0000...0000` for the first |
| `payload` | object | One of the payload types in §5 |
| `signature` | base64 string | Ed25519 over JCS(envelope minus signature) |

### 4.3 Server validation sequence

1. Verify mTLS client certificate → device_id from cert serial.
2. Verify `device_id` in body matches cert.
3. Verify `seq > last_accepted_seq` for this device → else `REPLAY_DETECTED`.
4. Verify `prev_hash` matches stored `payload_hash` of `seq - 1` → else
   `CHAIN_BROKEN`.
5. Verify `payload_hash == SHA-256(JCS(payload))` → else `INVALID_SIGNATURE`.
6. Verify Ed25519 signature with the device public key → else `INVALID_SIGNATURE`.
7. Accept. Record server receive timestamp as authoritative time (SR-C2).

### 4.4 Server response

Body: `ServerResponse` (see `schemas/server_response.jsonschema`)

The server may return **only**:
- `accepted` (boolean)
- `seq_ack` (the seq it stored)
- `error_code` (null on success)
- `config_version` (integer; agent re-fetches config if changed)
- `revoked` (boolean; agent stops immediately if true)

**No field may carry a command, a script, a URL to fetch, or any executable
instruction. This is a schema-level guarantee, not a policy (SR-E1).**

---

## 5. Payload types

Each payload type is defined by a JSON schema in `docs/schemas/`.

### 5.1 Heartbeat

File: `schemas/heartbeat.jsonschema`  
Sent every 60 seconds. ~200 B per event (FR28).

```json
{
  "type": "heartbeat",
  "uptime_seconds": 3600,
  "link_type": "ethernet",
  "link_speed_mbps": 100
}
```

### 5.2 MeasurementResult

File: `schemas/measurement_result.jsonschema`  
Carries network metrics. **A failed measurement is still a result and must be
sent** (FR13). `failure_reason` is set on failure; metric fields are null.

Only network metrics appear here. No CPU, memory, disk, file, browser, user
or any non-network field may ever be added (PR1, PR2).

```json
{
  "type": "measurement_result",
  "measured_at": "2027-03-01T10:05:00Z",
  "measurement_kind": "latency_jitter_loss",
  "latency_ms": 12.4,
  "jitter_ms": 1.1,
  "packet_loss_pct": 0.0,
  "throughput_download_mbps": null,
  "throughput_upload_mbps": null,
  "bytes_used": 1024,
  "failure_reason": null,
  "link_type": "ethernet",
  "link_speed_mbps": 100
}
```

### 5.3 SpeedtestLeaseRequest / SpeedtestLeaseResponse

Files: `schemas/speedtest_lease_request.jsonschema`,
`schemas/speedtest_lease_response.jsonschema`

The agent **must** obtain a lease before running a full speedtest (FR30). The
server grants at most one active lease per office at a time.

Request:
```json
{ "type": "speedtest_lease_request", "nic_utilisation_pct": 3.2 }
```

Response (granted):
```json
{
  "type": "speedtest_lease_response",
  "decision": "granted",
  "lease_token": "tok_abc123",
  "retry_after_seconds": null
}
```

Response (refused — office busy):
```json
{
  "type": "speedtest_lease_response",
  "decision": "refused_busy",
  "lease_token": null,
  "retry_after_seconds": 120
}
```

### 5.4 FailureReport

File: `schemas/failure_report.jsonschema`  
Sent when a measurement fails or when the agent reconnects after an outage.

```json
{
  "type": "failure_report",
  "failed_at": "2027-03-01T10:05:00Z",
  "reason": "transport_failure",
  "duration_ms": 45000
}
```

### 5.5 LifecycleEvent

File: `schemas/lifecycle_event.jsonschema`  
Boot and shutdown markers. The server uses these to distinguish a lost
connection from a computer that was switched off (FR14).

```json
{ "type": "lifecycle_event", "event": "SHUTDOWN", "event_at": "2027-03-01T18:00:00Z", "boot_id": null }
```

```json
{ "type": "lifecycle_event", "event": "BOOT", "event_at": "2027-03-02T08:00:00Z", "boot_id": "b001" }
```

**Classification rule:**
- `SHUTDOWN` then `BOOT` with a gap → **powered off**. Not an outage.
- `BOOT` with no preceding `SHUTDOWN`, or continuous uptime with missed
  heartbeats → **outage**, duration recorded.
- The server corroborates against other computers in the same office before
  publishing (TECHNICAL-PLAN §2.6).

### 5.6 RevocationNotice

File: `schemas/revocation_notice.jsonschema`  
Server-to-agent only. The agent stops submitting immediately and marks itself
revoked in local state (SR-A5).

---

## 6. The offline spool

When the agent cannot reach the server it stores results locally in a
**hash-chained spool** (SR-E5).

### 6.1 Spool record layout

```
{ seq, ts_mono, ts_wall, payload, prev_hash, hmac }
```

| Field | Description |
|---|---|
| `seq` | Same seq that will appear in the envelope |
| `ts_mono` | Monotonic clock ticks (never adjusted by NTP) |
| `ts_wall` | Wall clock at measurement time |
| `payload` | Serialised payload object |
| `prev_hash` | `payload_hash` of the previous spool record |
| `hmac` | HMAC-SHA-256(device_key, record_without_hmac) |

Editing any record breaks the HMAC → server rejects the tampered chain
(SR-E5, S5).

### 6.2 Drain order

On reconnect, the agent replays spooled envelopes in `seq` order. The server
processes them in order and records the server receive timestamp for each. The
agent's `ts_wall` is stored as advisory data alongside the authoritative server
time (SR-C2).

---

## 7. Error codes

| Code | Meaning |
|---|---|
| `REPLAY_DETECTED` | `seq <= last_accepted_seq` |
| `SEQ_GAP` | `seq` skipped a value (gap in chain) |
| `INVALID_SIGNATURE` | Signature verification failed or `payload_hash` mismatch |
| `REVOKED` | Device certificate has been revoked |
| `CHAIN_BROKEN` | `prev_hash` does not match stored previous hash |
| `UNSUPPORTED_VERSION` | `protocol_version` not supported by this server |
| `RATE_LIMITED` | Request rate exceeded abuse limit (SR-G6) |
| `INTERNAL_ERROR` | Unexpected server error; agent should retry with backoff |

---

## 8. What the server must never send

The server response schema (`schemas/server_response.jsonschema`) and the
revocation notice (`schemas/revocation_notice.jsonschema`) are the only
server-to-agent messages. Together they may carry only:

- an acceptance flag and seq acknowledgement
- an error code
- a config version number (no content, just a version integer)
- a revocation flag

**The server must never send:**
- A shell command or script
- A URL to fetch or execute
- A file path or file content
- Any instruction that causes the agent to take an action beyond
  stopping, retrying or fetching a config version

This constraint is enforced at the schema level. There is no config option
to relax it (SR-E1).

---

## 9. Acceptance criteria for PRs that depend on this spec

A PR for issue #1 (agent), #2 (backend API), or #3 (database) must:

1. Reference the message type schemas in `docs/schemas/`.
2. Include at least one unit test using the test vectors in
   `docs/test_vectors.md`.
3. Not invent a new message field without a corresponding schema update and
   a PR against this document.
4. Produce and verify JCS-canonical signatures, not ad-hoc serialisation.
