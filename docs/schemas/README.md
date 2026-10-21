# HEIMDALL Protocol Schemas

This directory contains the JSON schemas for every message in the HEIMDALL wire protocol, version 1.

## Contents

| Schema | Message type |
|---|---|
| `enrol-request.schema.json` | Agent → Server: enrolment request with office code and CSR |
| `enrol-response.schema.json` | Server → Agent: enrolment response (enrolled, refused, pending) |
| `envelope.schema.json` | The signing envelope that wraps every post-enrolment message |
| `heartbeat.schema.json` | Payload: 60-second heartbeat proving the agent is alive |
| `measurement.schema.json` | Payload: throughput + latency + jitter + loss measurement result |
| `lease-request.schema.json` | Payload: speedtest lease request |
| `lease-response.schema.json` | Server → Agent: speedtest lease granted or refused |
| `failure-report.schema.json` | Payload: a failed measurement is still a result (FR13) |
| `spool-record.schema.json` | Hash-chained local spool record for offline storage |
| `submit-response.schema.json` | Server → Agent: response to any POST /v1/submit |

## Example messages

Every schema has one or more example messages in the `examples/` directory, which are used in the CI validation test. If you're implementing the agent or backend, these examples show you what a real message looks like.

## Using these schemas

### In your implementation

If you're implementing the agent or backend:

1. **Read the protocol spec first**: `docs/PROTOCOL.md` is the authoritative document. The schemas are derived from it.
2. **Copy the examples**: Start with the example messages in `examples/` and use them as test fixtures.
3. **Validate your messages**: Load the appropriate schema and validate your serialised JSON against it before you try to send it. Catching schema violations before you transmit them is cheaper than debugging wire-level errors.

### In CI

The `.github/workflows/validate-schemas.yml` workflow runs automatically on every push that touches this directory. It validates all examples against their schemas. If the workflow fails, the examples do not match the schemas and something is wrong.

### Locally

To validate schemas against examples:

```bash
cd docs/schemas
npm install
npm run validate
```

Exit code 0 = all examples valid. Non-zero = at least one example is invalid.

## Adding a new message type

1. Write the JSON schema file and save it in this directory.
2. Add one or more example messages to `examples/`.
3. Update `validate-examples.js` to map the example filename prefix to the schema file.
4. Run `npm run validate` locally to prove the examples are valid.
5. Update `docs/PROTOCOL.md` to document the new message type.
6. Commit the schema, the examples, and the protocol doc together.

## Naming conventions

- Schema files: `<message-type>.schema.json`
- Example files: `<message-type>.example.json`, or `<message-type>-<variant>.example.json` if there are multiple variants (e.g. `enrol-response-enrolled.example.json`, `enrol-response-refused.example.json`)

## Test vector

The worked example in §10 of `docs/PROTOCOL.md` includes a normative test vector — a specific keypair, a specific payload, and the exact signature that both sides must produce. Use this to verify your signing and verification implementations. If your signature doesn't match the test vector, your crypto is wrong.

---

**Acceptance criterion:** A JSON schema file per message type, committed, and CI validates examples against them. This directory and its CI workflow satisfy that criterion.
