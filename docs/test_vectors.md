# HEIMDALL Protocol — Test Vectors

These vectors let any independent implementer verify that their signing and
serialisation logic matches HEIMDALL's. Both the agent side and the server side
must pass these before their PRs can be reviewed.

---

## Vector 1 — Signing a heartbeat envelope

### Input

```
device_id : "a1b2c3d4-0000-0000-0000-000000000001"
seq       : 42
nonce     : "deadbeefdeadbeefdeadbeefdeadbeef"
ts_agent  : "2027-03-01T10:00:00Z"
payload   : {"type":"heartbeat","uptime_seconds":3600}
prev_hash : "0000000000000000000000000000000000000000000000000000000000000000"
```

### Step 1 — Canonical payload hash

Serialise the payload with **JCS** (RFC 8785 — keys sorted, no extra
whitespace):

```
{"type":"heartbeat","uptime_seconds":3600}
```

SHA-256 of that UTF-8 string:

```
payload_hash = e3b60f...  (compute with: echo -n '{"type":"heartbeat","uptime_seconds":3600}' | sha256sum)
```

### Step 2 — Envelope to sign

Construct the envelope object (all fields except `signature`), serialise with
JCS, then sign with Ed25519.

```json
{
  "device_id": "a1b2c3d4-0000-0000-0000-000000000001",
  "nonce": "deadbeefdeadbeefdeadbeefdeadbeef",
  "payload": {"type": "heartbeat", "uptime_seconds": 3600},
  "payload_hash": "<sha256 from step 1>",
  "prev_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "protocol_version": 1,
  "seq": 42,
  "ts_agent": "2027-03-01T10:00:00Z"
}
```

Note: JCS sorts keys alphabetically. The order above is the canonical order.

### Step 3 — Ed25519 signing

```
private_key (hex) : (your test keypair — generate with: openssl genpkey -algorithm ed25519)
message           : JCS bytes from Step 2
signature         : Ed25519Sign(private_key, message)  →  base64-encode
```

### Acceptance criterion

Both the agent implementation and the server implementation must:

1. Produce the same `payload_hash` for the same payload bytes.
2. Produce the same JCS byte sequence for the same envelope fields.
3. Verify the signature correctly with the corresponding public key.

Include this vector in your unit tests. Reference it with:
`// test vector: docs/test_vectors.md#vector-1`

---

## Vector 2 — Replay rejection

Given two envelopes from the same `device_id`:

| envelope | seq |
|---|---|
| A | 10 |
| B | 10 |

The server must reject envelope B with `error_code: "REPLAY_DETECTED"`.

---

## Vector 3 — Chain break detection

Given three envelopes from the same `device_id`:

| envelope | seq | prev_hash |
|---|---|---|
| A | 1 | `0000...0000` |
| B | 2 | `hash(A.payload_hash)` |
| C | 3 | `0000...0000`  ← wrong |

The server must reject envelope C with `error_code: "CHAIN_BROKEN"`.
