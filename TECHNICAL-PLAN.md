# HEIMDALL — Technical Plan & Cost Model

> Companion to `HEIMDALL-PRD.md`. The PRD says *what* and *why*. This says *how*, *when*, and *what it costs*.
>
> **Status:** draft v0.1 — architecture proposal, not yet agreed.
> **Assumed pilot date:** 16 September **2027** (PRD says 2026, which is today; see §0).

---

## 0. Assumptions this plan rests on

| # | Assumption | Impact if wrong |
|---|---|---|
| A1 | Pilot launch is 16 Sep **2027** (12-month runway), not 2026 | Whole timetable (§7) shifts |
| A2 | Pilot scale: Eastern Province, ~40 offices, ~250 computers | Cost model §8 |
| A3 | National scale (year 3): ~2,000 offices, ~15,000 computers | Cost model §9 |
| A4 | Office computers are predominantly **Windows 10/11**, some Ubuntu | Agent packaging priority |
| A5 | Team is 2 paid engineers + owner, or equivalent volunteer effort | Cost model §8.1 |
| A6 | Servers hosted outside Sri Lanka (EU/SG) for the API; **measurement probes inside Sri Lanka** | Latency validity + bandwidth cost |
| A7 | Currency: USD, LKR at 300/USD | Cost tables |

---

## 1. System shape

```
┌────────────────────┐        ┌──────────────────┐       ┌───────────────────┐
│  Office computer   │        │   Edge (CDN/WAF) │       │   Public map +    │
│  ┌──────────────┐  │        │                  │       │   archive (static)│
│  │ heimdall-agent│ │──mTLS──▶  ingest.heimdall ├──────▶│   map.heimdall.lk │
│  │ (Go, service) │ │        │  enrol.heimdall  │       └───────────────────┘
│  └──────┬───────┘  │        │  api.heimdall    │                 ▲
│         │ UDP/TCP  │        │  admin.heimdall  │                 │ cached JSON
└─────────┼──────────┘        └────────┬─────────┘                 │
          │                            │                  ┌────────┴────────┐
          ▼                            ▼                  │  read API (RO)  │
 ┌──────────────────┐        ┌──────────────────┐         └────────┬────────┘
 │ Probe nodes (LK) │        │ ingest service   │                  │
 │ latency/jitter/  │◀──────▶│ (stateless, Go)  │                  │
 │ loss + throughput│        └────────┬─────────┘                  │
 └──────────────────┘                 │ NATS                       │
                                      ▼                            │
                            ┌──────────────────┐         ┌─────────┴────────┐
                            │ validator worker │────────▶│ PostgreSQL +     │
                            │ (anomaly, SR-C3) │         │ TimescaleDB      │
                            └──────────────────┘         │ append-only      │
                                      │                  └─────────┬────────┘
                                      ▼                            │ nightly
                            ┌──────────────────┐         ┌─────────▼────────┐
                            │ Merkle anchoring │         │ Open-data export │
                            │ (daily root, TSA)│         │ Parquet → R2     │
                            └──────────────────┘         └──────────────────┘
```

Four **separate** deployments, separate credentials, separate network policy: `enrol`, `ingest`, `public-read`, `admin`. Satisfies SR-G5 (public view cannot mutate data).

---

## 2. The agent

### 2.1 Language and packaging
- **Go**, single static binary, cross-compiled. No runtime to install, no interpreter, tiny attack surface, easy reproducible builds (SR-F2).
- **Windows:** MSI (WiX) installing a Windows Service running as `NT SERVICE\heimdall` (virtual account, no admin rights after install). Authenticode-signed with EV cert.
- **Linux:** `.deb` + `.rpm`, systemd unit, dedicated `heimdall` user, `ProtectSystem=strict`, `NoNewPrivileges=yes`, `CapabilityBoundingSet=` (empty).
- **macOS:** deferred to post-pilot.

### 2.2 Identity (SR-A1, SR-A2, SR-A6)
1. Installer asks for the **office code** only.
2. Agent generates an **Ed25519 keypair on the machine**. Private key never leaves it.
   - Windows: CNG key, **TPM-backed** where available (`Microsoft Platform Crypto Provider`), else DPAPI machine scope.
   - Linux: TPM2 sealed where available, else `0600` file owned by `heimdall`.
3. Agent sends a CSR + office code + machine fingerprint to `enrol`.
4. Server issues a **device certificate** (private CA, `step-ca`) valid 90 days, auto-renewed over mTLS. Device ID = cert serial.
5. **Machine fingerprint** = HMAC(pepper, stable hardware IDs: SMBIOS UUID + machine GUID + disk serial). Peppered server-side, never published. Same fingerprint re-enrolling → recorded re-enrolment, not a new computer (SR-A6). Same fingerprint appearing on a second device ID concurrently → clone, both refused (SR-A2).

Key point: **the office code is a one-shot enrolment ticket, never a submission credential.** Every result is authenticated by the device certificate + signature.

### 2.3 Office codes (FR2, SR-A3)
- Format `HMD-4K7P-Q2XR-9F3T` — Crockford base32 (no I/L/O/U, case-insensitive), 60 bits entropy + 4-bit checksum. Readable over a phone, writable by hand, not guessable.
- Stored **Argon2id-hashed**. Carries: issuing office, `max_enrolments`, `valid_until`, `revoked_at`.
- Rate limit: 5 wrong attempts per source /24 per hour, then exponential block. Enrolment beyond `max_enrolments` → **held for maintainer approval**, not refused silently (SR-A4, FR5).

### 2.4 What the agent measures (FR10)

| Measurement | Method | Cadence | Data per event |
|---|---|---|---|
| **Heartbeat / availability** | 1 signed UDP datagram to probe + TCP fallback | 60 s | ~200 B |
| **Latency + jitter + loss** | 20-packet UDP echo train to nearest LK probe | 5 min | ~2 KB |
| **Throughput (down/up)** | RMBT-style multi-stream TCP against LK probe, **duration-capped 8 s**, adaptive stream count | 1×/day per computer, **office-serialised** | 10–60 MB |
| **Link context** | Interface type (wifi/eth), negotiated link speed, MTU, DNS resolver class, ASN of public IP | on change | <1 KB |

Throughput uses **duration caps, not byte caps** — a 2 Mbps office transfers 2 MB, a 100 Mbps office transfers 100 MB, both in 8 s. That is FR29 ("scaled to the size of the connection") for free.

### 2.5 Not slowing the office down (FR27–FR33)
- **Office-level speedtest lease.** Agent must `POST /lease/speedtest` and receive a token before testing. Server grants **at most one live lease per office** (FR30) and refuses if the office's **daily byte ceiling** is spent (FR32). Ceiling enforced twice: server refuses the lease, agent refuses to start.
- **Busy-link back-off (FR31):** before testing, agent samples NIC counters for 10 s. If throughput >10% of last-known capacity, defer and retry in the next window. Preference window 22:00–06:00 local, biased but not exclusive (need daytime samples too).
- **Own usage published (FR33):** every agent reports its own bytes sent/received; the office page shows "HEIMDALL used X MB this month".

### 2.6 Outage vs powered off (FR14, FR12, SR-E5)
- Agent keeps a **hash-chained local spool**: each record `{seq, ts_mono, ts_wall, payload, prev_hash}`, HMAC'd with the device key. Editing any record breaks the chain → server rejects (S5).
- **Clean shutdown marker**: the service stop handler writes a signed `SHUTDOWN{ts}` record. On next boot the agent writes `BOOT{ts, boot_id}`.
  - `SHUTDOWN` then `BOOT` with a gap → **powered off**. Not an outage.
  - `BOOT` with no preceding `SHUTDOWN`, or continuous uptime with failed heartbeats → **outage**, with duration.
  - Unclean power loss looks like an outage from one machine, so the server **corroborates against the other computers in the same office** before publishing.
- Server independently records heartbeat gaps; agent claim and server observation must agree or the interval is marked `disputed`.

### 2.7 Presence at the office (FR7–FR9, SR-D1–D3, PR4)
No GPS, no coordinates, ever. The agent reports a **network locality fingerprint**, all values hashed with a per-office pepper server-side:

1. Public IP **/24 prefix + ASN** (the /32 is discarded on ingest, never stored — PR4).
2. **Default gateway MAC**, salted-hashed.
3. **First 3 traceroute hops**, salted-hashed, as an ordered tuple.
4. **RTT profile** to the nearest LK probe (median + variance over 24 h).
5. LAN subnet CIDR (RFC1918 only).

The office's **reference fingerprint** is derived by quorum from its already-verified computers. A new or moved computer must match **≥3 of the 5** signals to be `on_site`. Fewer → `off_site`: results are stored, flagged, and **excluded from the office's public figures** (FR8). A VPN or routed-home connection (SR-D2) fails signals 2, 3 and 5 simultaneously, and typically shows an RTT profile inconsistent with the office's.

### 2.8 Safety of the office computer (SR-E1–E6)
- **No command channel exists in the protocol.** The server can return only: a lease grant/denial, a config *version number*, and a "you are revoked" flag. There is no field that carries a command, a URL to fetch, or a script. This is a schema-level guarantee, not a policy (SR-E1).
- **No listening socket.** Agent binds nothing; every connection is outbound (SR-E2). Verified by a CI test that asserts zero listening sockets after start.
- Runs as an unprivileged service account after install (SR-E3). Touches no firewall, AV or policy setting (SR-E4).
- Uninstall removes binary, spool, keys and registry/unit entries; leaves a single audit line in the server record (SR-E6).

---

## 3. Transport and result integrity

### 3.1 In transit (SR-B1–B4)
- **TLS 1.3 only**, mutual TLS with the device certificate.
- **Public-key pinning to the HEIMDALL intermediate CA** (pin the intermediate, not the leaf, so leaf rotation is free). Two pins live at all times (current + next) for rotation.
- **No fallback path exists in the code.** No plaintext, no TLS 1.2, no "skip verify" flag, not even behind a debug build tag. A handshake failure is spooled locally and reported as a `transport_failure` event once connectivity returns (SR-B3, SR-B4).
- A blocked or MITM'd agent therefore produces **silence plus a later failure record** — never a healthy-looking result (S1, S2).

### 3.2 Result integrity (SR-C1, SR-C2)
Every measurement is signed **at the point of measurement**, before it touches disk:

```
payload  = JCS-canonical JSON of the measurement
envelope = { device_id, seq, nonce, ts_agent_wall, ts_agent_mono, payload_hash, prev_hash }
signature = Ed25519(device_key, envelope)
```

- `seq` is strictly increasing per device. Server rejects `seq <= last_seq` → **no replay** (S6).
- `ts_agent_wall` is advisory; the **server's receive timestamp is authoritative** and the agent's clock skew is recorded. Back-dating is impossible because the published timeline uses server time (SR-C2).
- `prev_hash` chains the device's whole history — a gap or a fork is detectable.

### 3.3 Anomaly quarantine (SR-C3)
Validator worker rejects-to-quarantine, not to public:
- throughput exceeding the office's observed 99th percentile by >3× with no link-speed change;
- byte-identical repeated results;
- correlated step-changes across many offices in the same minute (suggests a server-side or probe fault, not a network truth);
- impossible physics (throughput > negotiated link speed).
Quarantined rows are stored, visible to maintainers, invisible to the public until reviewed. The review decision is itself an audit record.

### 3.4 Append-only archive (SR-C4, SR-C5)
- `results` is an **insert-only** hypertable. The application DB role is granted `INSERT, SELECT` only — no `UPDATE`, no `DELETE`. Corrections are new rows in `corrections` referencing the original, which stays.
- Nightly job builds a **Merkle tree over the day's result hashes**, publishes the root to:
  1. a public, append-only `heimdall-roots` git repository (signed commits), and
  2. an RFC 3161 timestamp authority.
- `GET /verify/{result_id}` returns the Merkle inclusion proof. **Anyone can prove a result is in the archive and that the archive was not rewritten** (SR-C5, S11).

---

## 4. Backend

| Concern | Choice | Why |
|---|---|---|
| API services | **Go** (`net/http` + `chi`), 4 separate binaries | Same language as agent; shared protocol types; small images |
| Datastore | **PostgreSQL 17 + TimescaleDB** | Time-series hypertables + continuous aggregates + ordinary SQL for the relational half (offices, devices, audit) — one system, not two |
| Rollups | Timescale continuous aggregates: 5 min / 1 h / 1 day | Map reads never touch raw rows |
| Queue | **NATS JetStream** | Ingest decoupled from validation; survives validator restarts |
| Object store | **Cloudflare R2** | Zero egress fees for open-data downloads (FR24) |
| Edge | **Cloudflare** — WAF, rate limits, cache, DDoS | SR-G6, SR-G7 at near-zero cost |
| Private PKI | **step-ca** | Device certs, short-lived, automated renewal |
| Secrets | SOPS + age in repo for config shape, real values in host KMS / Doppler | SR-G4 |
| IaC | **Terraform** + Ansible | Reproducible rebuild; disaster recovery is a `terraform apply` |
| Observability | Prometheus + Grafana + Loki (self-host), Sentry | Cost control |

### 4.1 Retention
Raw results **kept in full, indefinitely** (PRD §14.1). Compressed by Timescale native compression after 30 days — roughly 10–20× on this shape of data. At national scale that is ~40 GB/year compressed. Storage is not a constraint here; keep everything.

### 4.2 "Typical, not best" (FR22)
Every published performance figure is a **median with p10/p90 band** over the window, never a maximum. Enforced in the aggregate definitions so no view can accidentally publish a peak.

---

## 5. Public map and archive

- **Next.js static export** + **MapLibre GL JS**. Basemap: **Protomaps** tiles self-hosted from R2 (one-off ~5 GB for Sri Lanka), so there is **no per-view map bill and no third-party tracking of viewers**.
- Read API is a separate read-only service, edge-cached 60 s for live status, 24 h for historical ranges.
- Live view: office pin coloured by current status; click → office panel with live status, 24 h/7 d/30 d medians, per-computer anonymised rows ("Computer 3"), coverage `enrolled / limit` (FR6a), HEIMDALL's own data use (FR33).
- Archive: date-range picker, any office, any window, permalinked URLs (FR19).
- Enrolment history timeline, anonymised, dated (FR21).
- Offices with no reporting computer render as an explicit **"silent"** pin — a distinct colour, never omitted (FR23).
- Open data (FR24): daily Parquet + CSV on R2, plus the Merkle root file, under **ODbL** or **CC-BY-4.0** (owner to decide).

### 5.1 Anonymity for small offices (SR-H1)
A one-computer office makes "Computer 1" equal to "that person's machine". Mitigation: per-computer breakdown is shown only when an office has **≥3 enrolled computers**. Below that, only office-level aggregates are published. Computer labels are **randomly assigned and stable**, never install-ordered (install order leaks which desk was set up first).

---

## 6. Release, updates, maintainers

### 6.1 Supply chain (SR-F1–F7)
- Builds in **GitHub Actions**, hermetic, `-trimpath`, `CGO_ENABLED=0`, pinned toolchain → **byte-reproducible**. A `verify-build` workflow rebuilds each release and diffs; anyone can run the same script (SR-F2).
- Artefacts signed with **cosign** (keyless, Sigstore/Fulcio) + **SLSA v1.0 provenance** attestation (SR-F1).
- Release requires a **GitHub protected environment with 2 required reviewers** from the maintainers team. One stolen account cannot ship (SR-F5, S8).
- Update channel: **TUF** (`go-tuf`) signed metadata. Agent refuses any target failing verification (SR-F3) and **refuses any version lower than its own** — version floor recorded in TUF metadata and in the agent's own state (SR-F4, S7).
- Windows binaries additionally **Authenticode-signed with an EV certificate on a hardware token** (needed for SmartScreen reputation).
- Dependencies: `govulncheck` + Dependabot + `osv-scanner` in CI; new direct dependencies require a reviewed PR (SR-F6).
- Key hierarchy: offline TUF root key on 2 hardware tokens in separate physical custody; online snapshot/timestamp keys in KMS; documented rotation runbook (SR-F7).

### 6.2 Maintainer access (SR-G1–G3)
- Admin console behind **Cloudflare Access + WebAuthn hardware keys**. No password-only route exists (SR-G1).
- Roles, separately grantable (SR-G2): `institution-approver`, `device-manager`, `release-publisher`, `data-reviewer`, `auditor` (read-only).
- **Audit log:** append-only table, hash-chained, streamed to write-once object storage with object-lock. Maintainers cannot erase it, including the admins (SR-G3). Every approval, revocation, correction, release and quarantine decision lands there.
- Revocation is instant: device cert added to the CRL/OCSP deny list, `ingest` checks the deny list on every request (SR-A5).

---

## 7. Roadmap — 12 months to pilot

| Phase | Window | Deliverable | Security gate |
|---|---|---|---|
| **P0 — Foundations** | M0–M1 | Repo, licence, SECURITY.md, threat model doc, CI skeleton, reproducible build proven, Terraform for staging | SR-I1 published |
| **P1 — Protocol & identity** | M1–M3 | Protocol spec + JSON schemas, enrol service, step-ca, device cert issue/renew, office-code lifecycle, admin API for approve/revoke | **SR-A1–A5, SR-B1–B3** |
| **P2 — Agent core** | M3–M5 | Go agent: heartbeat, latency/jitter/loss, spool + hash chain, boot/shutdown markers, Windows service + MSI, Linux packages | **SR-C1, C2, SR-E1–E6** |
| **P3 — Measurement plane** | M5–M6 | LK probe nodes, RMBT throughput, office lease server, daily ceiling, back-off | FR27–FR33 verified on a real slow link |
| **P4 — Data plane** | M6–M7 | Timescale schema, ingest + validator, anomaly rules, Merkle anchoring, verify endpoint | **SR-C3, C4, C5** |
| **P5 — Presence** | M7–M8 | Locality fingerprint, quorum reference, on/off-site classification, VPN-evasion tests | **SR-D1–D3** |
| **P6 — Public map** | M8–M9 | Map, office pages, archive browse, open-data export, coverage + silence rendering | **SR-H1, H2** |
| **P7 — Release chain** | M9–M10 | TUF update channel, cosign + SLSA, EV signing, 2-reviewer gate, downgrade tests | **SR-F1, F3, F4, F5** |
| **P8 — Hardening** | M10–M11 | S1–S11 scenario test suite in CI, load test, incident plan, runbooks, independent review begins | **SR-I3**, all §11.5 gates |
| **P9 — Field trial** | M11–M12 | 3 friendly offices, ~15 computers, 6 weeks real running, fix what reality breaks | §11.5 gates re-verified |
| **PILOT** | M12 (16 Sep 2027) | Eastern Province rollout via Chief Secretary | All §11.5 met, no blockers |

**Post-pilot:** independent security review findings resolved (SR-I5) → letters to other provinces → national scale-out.

### 7.1 Attack-scenario tests as CI
S1–S11 from PRD §11.4 become an automated suite — a MITM proxy test, a fake-server test, a clone-identity test, a replay test, a spool-tamper test, a downgrade test, a flood test. A failing scenario fails the build. This is what makes §11.5 a gate rather than a wish.

---

## 8. Cost — pilot (~40 offices, ~250 computers)

### 8.1 One-time

| Item | USD | LKR @300 | Notes |
|---|---:|---:|---|
| Domains (`.lk` + `.org`, 2 yr) | 120 | 36,000 | |
| **EV code-signing certificate + HW token** (1 yr) | 400 | 120,000 | Required for Windows SmartScreen. Non-negotiable |
| Apple Developer (if macOS agent) | 99 | 29,700 | Deferrable |
| Hardware security keys ×5 (YubiKey 5) | 250 | 75,000 | TUF root + maintainer MFA |
| **Independent security review** (SR-I5) | 10,000 | 3,000,000 | Local firm/CERT ≈ $3–8k; international ≈ $15–25k. Budgeted mid |
| Legal / privacy notice / MoU drafting | 1,500 | 450,000 | Often pro bono; keep as a line |
| Protomaps SL basemap build + misc setup | 100 | 30,000 | |
| Contingency 15% | 1,860 | 558,000 | |
| **Total one-time (infra + audit)** | **~14,300** | **~4,290,000** | |
| *Engineering, if paid* | *54,000–90,000* | *16.2M–27M* | 2 engineers × 9 months @ $3–5k/mo |
| *Engineering, if volunteer/owner-led* | *0* | *0* | Realistic for an open-source civic project |

### 8.2 Monthly (pilot)

| Item | USD/mo | LKR/mo |
|---|---:|---:|
| App + DB server (Hetzner CCX23, 4 vCPU ded / 16 GB) | 27 | 8,100 |
| DB replica / standby (CPX31) | 15 | 4,500 |
| Probe + speedtest nodes, **Colombo ×2** | 90 | 27,000 |
| Probe node, Singapore (regional reference) | 12 | 3,600 |
| Cloudflare Pro (WAF rules, analytics) | 20 | 6,000 |
| R2 object storage + open-data hosting | 3 | 900 |
| Backups (Storage Box 1 TB, off-provider copy) | 8 | 2,400 |
| Transactional email (Resend/Postmark) | 0–15 | 0–4,500 |
| Monitoring (self-hosted; Sentry free tier) | 0 | 0 |
| Amortised code-signing + domains | 43 | 12,900 |
| **Total** | **~220** | **~66,000** |

Pilot traffic is negligible: 250 computers × ~30 MB/day of speedtest ≈ **225 GB/month**, well inside included bandwidth. **The pilot's real cost is the security audit, not the servers.**

---

## 9. Cost — national (~2,000 offices, ~15,000 computers)

### 9.1 Load
- Heartbeats: 15,000 / 60 s = **250 req/s** sustained. Trivial for Go + NATS.
- Results: ~15,000 × 300/day ≈ **4.5 M rows/day**, ~50 GB/yr compressed.
- Speedtest bandwidth: office-serialised, each computer every 3rd day → 2,000 offices × 3 tests/day × ~30 MB ≈ **180 GB/day ≈ 5.4 TB/month**, concentrated on the LK probe nodes. This is the single biggest scaling cost and the one worth attacking with peering.

### 9.2 Monthly (national)

| Item | USD/mo | LKR/mo |
|---|---:|---:|
| API servers ×3 (CCX33) | 165 | 49,500 |
| Timescale primary (CCX43) + replica | 240 | 72,000 |
| **LK probe/speedtest nodes ×5** (bandwidth-heavy) | 400 | 120,000 |
| Regional probes ×2 (SG/IN) | 30 | 9,000 |
| Bandwidth overage buffer | 100 | 30,000 |
| Cloudflare Pro + Load Balancing | 45 | 13,500 |
| R2 + backups (versioned, object-lock) | 35 | 10,500 |
| Monitoring (Grafana Cloud paid / self-host uplift) | 50 | 15,000 |
| Amortised certs, domains, KMS | 55 | 16,500 |
| **Total** | **~1,120** | **~336,000** |

Annual recurring on top of the above: EV cert renewal ~$400, domains ~$120, **security re-review ~$6,000/yr** (SR-I4 requires yearly threat-model review; a paid re-test is the honest way to do it).

### 9.3 The bandwidth lever
The LK probe nodes dominate national cost. Three ways to cut it, in order of value:
1. **Peer at LKIX** and/or host probes inside SLT/Dialog/LEARN networks — turns paid transit into settlement-free local traffic. Potentially removes ~$400/mo.
2. **Donated hosting** from ICTA / LEARN / a participating university. Realistic for a civic open-source project and worth asking before the pilot.
3. Stretch the per-computer full-test interval from 3 days to 7. Halves bandwidth; costs statistical resolution. Least attractive — measurement quality is the product.

---

## 10. Team

| Role | Pilot | National |
|---|---|---|
| Backend / agent engineer (Go) | 1.0 FTE | 1.0 |
| Frontend / map engineer | 0.5 | 0.5 |
| DevOps + security | 0.5 | 1.0 |
| Maintainer / institutional liaison (owner) | 0.5 | 1.0 |
| Community + support | — | 0.5 |

Minimum viable: **two committed engineers plus the owner.** Below that, the 12-month timetable does not hold and the security gates are where it will show first.

---

## 11. Principal risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Presence verification is defeatable** (§2.7) — network fingerprints are heuristics, not proof | High | Quorum-based reference, ≥3-of-5 matching, flag rather than silently trust, manual maintainer review of contested offices. Accept residual risk openly in the docs rather than overclaim |
| **Windows AV / SmartScreen blocks the agent** at scale | High | EV signing from day one, build install-base reputation during the field trial, submit to Microsoft and major AV vendors for allow-listing before pilot |
| **Offices on metered/paid data** are harmed by speedtests | Medium | Hard daily ceiling, duration-capped tests, published per-office usage, opt-down mode for known metered links |
| **Institutional adoption stalls** — the technical system works, nobody installs it | High | Out of product scope per PRD §5, but it is the real project risk. Field trial with 3 friendly offices exists to surface installation friction early |
| **Single maintainer bus factor** | High | Two-maintainer release gate forces a second person to exist; document runbooks from P0, not P8 |
| **Timescale/Postgres single point of failure** | Medium | Streaming replica + PITR (WAL-G) to a second provider; archive restore rehearsed quarterly |
| **Publishing real government weakness invites pushback** | Medium | Data is already committed as open and append-only. Governance and the MoU must set the expectation before the pilot, not after the first bad chart |

---

## 12. Immediate next steps (first 4 weeks)

1. Initialise the repo: MIT licence, `SECURITY.md` (SR-I1), `CODE_OF_CONDUCT.md`, ADR folder.
2. Write the **protocol specification** (`docs/PROTOCOL.md`) — message shapes, signing envelope, error codes. Everything else depends on it.
3. Prove the **reproducible build** in CI on day one; retrofitting it later is painful.
4. Stand up staging: one Hetzner box, `step-ca`, Postgres+Timescale, one Colombo probe.
5. Walking skeleton: agent enrols → sends one signed heartbeat → row lands in Timescale → renders on a map pin. End to end, nothing clever.
6. Confirm the **pilot date** (§0, A1) and the pilot office count (drives FR5 limits).
