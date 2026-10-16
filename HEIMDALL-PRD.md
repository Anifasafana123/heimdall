# HEIMDALL — Product Requirements Document

> **HEIMDALL: the guard at the bridge.**

**Owner:** Rumeasiyan · **Licence:** open source (MIT) · **Collaboration and support are welcome.**

> This document covers *what* HEIMDALL must do and *why*. It contains no technical or implementation decisions. Items marked **[PROPOSED — owner to decide]** are suggestions that have not been agreed.

---

## 1. What HEIMDALL is

HEIMDALL is an **open-source, public connectivity monitor for government institutions**. It is installed on computers in government offices. It measures the real internet connection those offices work on, continuously, and publishes the results openly on a map, live and as a historical archive, so anyone can see how well the government is connected.

It follows the example of **Giga**, the UNICEF–ITU initiative that maps school connectivity worldwide and publishes the results openly (https://maps.giga.global/map). HEIMDALL applies the same idea to **government offices**.

HEIMDALL is built and maintained as an independent open-source project and offered to the Government of Sri Lanka to use. It is not limited to one province: any government institution in the country may adopt it, and nothing in it prevents use elsewhere.

## 2. The name

**HEIMDALL** is the Norse watchman of the Bifröst bridge, who could see and hear across all realms.

**Tagline: "the guard at the bridge"** (owner, 16 Sep) — used everywhere: repository, map and documents. It carries both meanings at once: Heimdall guards the Bifröst, and a bridge is a real piece of network equipment. HEIMDALL watches every office's link to the network in the same way: continuously, openly, and without looking inside anything it guards.

## 3. The problem

Government services increasingly depend on a working internet connection. Nobody can see which government offices actually have one.

- Connectivity problems are known only when someone happens to report them.
- Offices with poor or failing connections that never report them stay invisible.
- The public has no view of the connectivity of the institutions that serve it.

## 4. Goals

1. **Measure real connectivity** at government offices, continuously: speed, quality and availability.
2. **Make it public.** Anyone can see the connectivity of any participating office, live and historically, as Giga does for schools.
3. **Show the full picture, honestly.** Every participating office and **every one of its computers** is enrolled and appears, including those that have stopped reporting, so aggregated figures are complete.
4. **Protect the people whose computers run it.** HEIMDALL measures the connection and nothing else.
5. **Be secure by design.** Security is HEIMDALL's first requirement, not a feature. Published results must be genuine, the software must never become a way into government computers, and no one must be able to fake, alter or erase the public record.

## 5. Non-goals

- **Not a questionnaire.** HEIMDALL measures; it does not ask.
- **Not staff monitoring.** It never looks at files, browsing, keystrokes, screens, or who uses a computer.
- **Not a repair tool.** It shows connectivity; it does not fix it.
- **Not a coordination tool.** How institutions are organised to install it is outside this product.

## 6. Who it serves

| Who | What they need |
|---|---|
| **The public** | An open view of government connectivity — live, historical, per office and per (anonymised) computer |
| **Participating institutions** | Simple installation, no daily effort, and confidence it does not watch their staff |
| **Staff whose computers run it** | No disruption; a clear, visible statement of what it measures |
| **Government decision-makers** | The same open data everyone else sees |
| **Maintainers** | Control over which institutions and computers may report, so the data stays trustworthy |
| **Contributors** | An open project they can improve and support |

## 7. Who can join

- **Any government institution** may request to join by **formal letter to the maintainers**.
- The maintainers approve the request and **issue that institution its office code**.
- **The maintainers decide how many computers** each office may enrol (§9).

## 8. How it is used

1. **Request.** An institution sends a formal letter to the maintainers.
2. **Issue.** The maintainers issue the institution an office code.
3. **Install.** Staff install HEIMDALL on an office computer and enter the office code once.
4. **Understand.** HEIMDALL shows **what it measures and what it never touches** before it starts.
5. **Measure.** It runs quietly and continuously in the background.
6. **Keep recording through outages.** While the connection is down it keeps a record, and reports the loss once the connection returns.
7. **Publish.** Results appear on the **public map**, live, and remain in the **public archive**.

## 9. Functional requirements

### Enrolment and office codes
- **FR1.** Each institution joins with an **office code issued by the maintainers** after a formal request. There are no self-registered or free-text offices.
- **FR2.** Office codes are **easy to read aloud and copy by hand** but **hard to guess**. Easy to use must not mean easy to misuse.
- **FR3.** Invalid, expired or revoked codes are rejected, with a clear message.
- **FR4.** The maintainers can **revoke or replace** an institution's code at any time, for example if it leaks.
- **FR5.** Each office's limit on enrolled computers **equals the number of computers that office actually has.** The limit exists so that computers outside the office cannot claim to belong to it. An attempt to enrol beyond the limit is refused and flagged to the maintainers. [PROPOSED — owner to decide: the institution states its computer count in its formal request, and any change comes through a new request.]
- **FR6.** Every enrolled computer is **individually registered** under its office, so the maintainers can see and remove any single computer.
- **FR6a.** **The aim is every computer in every participating office.** Aggregated office figures are only meaningful when all of an office's computers report, so the public view shows each office's coverage (enrolled computers against its limit).

### Making sure it measures the real office
- **FR7.** A result counts for an office **only when the computer is actually at that office**. HEIMDALL must recognise when a computer is somewhere else — a laptop taken home, or an installation made from a different area — and must **not** record that connection as the office's.
- **FR8.** A computer found measuring from outside its office is **flagged**, and its off-site results are excluded from the office's public record.
- **FR9.** Confirming that a computer is at its office must be done **without collecting or publishing anyone's precise location**.

### What is measured
- **FR10.** HEIMDALL measures **all four** of these:
  1. download and upload speed;
  2. response time (latency);
  3. connection stability (jitter and packet loss);
  4. availability (uptime and outages).
- **FR11.** It measures **continuously**, not as a one-off survey.
- **FR12.** **Outages are captured**, and reported once the connection returns.
- **FR13.** **A failed measurement is still a result.** It is recorded, never discarded.
- **FR14.** HEIMDALL distinguishes **a lost connection from a computer that is simply switched off**, so a switched-off computer is not published as an outage.

### Computers per office
- **FR15.** An office may run HEIMDALL on **several computers** (up to its limit).
- **FR16.** An office counts as **online if any one of its computers** is connected.

### Public results — live and archive
- **FR17.** Results are published on a **public map**, in the manner of Giga, viewable by **anyone, without an account**.
- **FR18.** The map shows each participating office's **live** connection status and recent performance.
- **FR19.** A **historical archive** lets anyone view past results for any office and any date range, **like an archive website**.
- **FR20.** Anyone can see a **per-computer breakdown** within each office. Computers appear **anonymised** (e.g. "Computer 3"), never by name, user, or other identifying detail.
- **FR21.** The public view shows **enrolment changes**, anonymised: computers added, computers that stopped reporting, and computers removed, with dates.
- **FR22.** An office's connection is described by its **typical performance, not its best moment**.
- **FR23.** Every participating office appears, including offices with **no computer currently reporting**. Silence is shown as silence, never hidden.
- **FR24.** **Open data.** **[PROPOSED — owner to decide]** Anyone can download the public results.

### Running in the office
- **FR25.** Installation and daily running require **no technical knowledge**.
- **FR26.** HEIMDALL runs **without disrupting** normal use of the computer.

### Impact on the office connection
- **FR27.** **HEIMDALL must not slow the office's internet in any way staff would notice.** Its own use of the connection is kept to the minimum needed to measure it.
- **FR28.** The small, regular "still connected" signal uses **negligible data**.
- **FR29.** Full speed tests, the only heavy part, are **short and infrequent**, and scaled to the size of the connection, so a slow office connection is never flooded.
- **FR30.** **Only one computer per office runs a full speed test at a time.** Computers in the same office never test together.
- **FR31.** HEIMDALL **backs off when the office connection is already busy**, and prefers quiet periods for full speed tests.
- **FR32.** Each office has a **daily ceiling on the data HEIMDALL may use**. It is never exceeded, which matters for offices on limited or paid data.
- **FR33.** HEIMDALL's own data use is **shown openly**, per office, so an institution can see it is not the cause of a slow connection.

## 10. Privacy requirements

- **PR1.** HEIMDALL measures **only the network connection** and the basic connection settings of the computer it runs on.
- **PR2.** It **never** collects files, documents, browsing history, keystrokes, screen content, or the identity of the person using the computer.
- **PR3.** It **never publishes** anything that identifies a staff member or a specific computer. Public per-computer data is anonymised (FR20).
- **PR4.** It **does not keep full network addresses or precise locations**, and publishes neither.
- **PR5.** The privacy boundary is **shown inside the app itself**, in plain language, not only in a separate document.

## 11. Security

**Security is HEIMDALL's highest-priority requirement.** HEIMDALL runs on government computers and publishes a public record the government and the public will rely on. A weakness here does more than break the product: it could fake the picture of government connectivity, or open a path into government machines. **Where security conflicts with convenience, security wins.**

### 11.1 What must be protected

| Asset | Why it matters |
|---|---|
| **The office computers** HEIMDALL runs on | They are government machines. HEIMDALL must never become a way in |
| **The integrity of the public record** | Faked or altered results would mislead the government and the public |
| **The archive** | History must never be silently rewritten or erased |
| **Office codes and enrolled computer identities** | These decide who can submit results |
| **The anonymity of staff and computers** | Public data must never lead back to a person |
| **The release and update channel** | Whoever controls updates controls every installed computer |
| **Maintainer access** | Maintainers approve institutions, computers and releases |
| **Availability** of the public map and archive | A public service must stay up |

### 11.2 Who might attack it

| Attacker | What they might try |
|---|---|
| **Someone on the office network or between the office and the internet** | Read, alter or block results in transit ("man-in-the-middle") |
| **An insider at an institution** | Make their office look better or worse; install on personal devices; run it from home |
| **Someone holding a leaked office code** | Enrol their own computers; submit invented results |
| **An attacker targeting the software supply** | Plant a malicious installer or update that reaches many government computers |
| **An attacker targeting the maintainers** | Take over a maintainer account to approve fake computers, alter records or push a release |
| **Someone mining the public data** | Re-identify staff or computers; learn when offices are unprotected |
| **A disruptor** | Flood the system to take the public map down |

### 11.3 Security requirements

**A. Enrolment and identity**
- **SR-A1.** An office code alone is **never enough** to submit results. It only starts enrolment of a specific computer, and that computer then gets its own identity.
- **SR-A2.** Each enrolled computer has a **unique identity that cannot be copied** to another computer. Cloned or duplicated installations are detected and refused.
- **SR-A3.** Office codes are **limited in use.** They have a validity period and a maximum number of enrolments, and cannot be reused once exhausted. Repeated wrong-code attempts are blocked.
- **SR-A4.** **Every new computer enrolment is visible to the maintainers.** Enrolments beyond the office's limit, or from unexpected places, are held for maintainer approval rather than accepted automatically.
- **SR-A5.** The maintainers can **revoke a single computer, or a whole office, instantly.** A revoked computer can no longer submit anything.
- **SR-A6.** Reinstalling on the same computer must not quietly create a second identity. Re-enrolment is recorded.

**B. Protection in transit (man-in-the-middle)**
- **SR-B1.** All communication between a computer and HEIMDALL is **private and tamper-proof**. Nobody on any network in between can read or change it.
- **SR-B2.** A HEIMDALL computer **verifies it is talking to the genuine HEIMDALL service** before sending anything. If it cannot, it sends nothing and records the failure.
- **SR-B3.** There is **no insecure fallback.** If secure communication is not possible, HEIMDALL does not "try anyway".
- **SR-B4.** An attacker who blocks traffic can cause silence but can never make silence look like a good connection. Blocked reports appear as missing data, not as healthy results.

**C. Integrity of results**
- **SR-C1.** Each result is **tamper-evident from the moment it is measured.** It cannot be edited on the computer before sending, or on the server afterwards, without detection.
- **SR-C2.** **No replay, no back-dating.** Old results cannot be re-sent, and results carry trustworthy times that cannot be forged.
- **SR-C3.** **Impossible or suspicious values** — speeds beyond what the connection could deliver, identical repeated results, sudden patterns across many computers — are flagged and kept out of the public view until reviewed.
- **SR-C4.** **The public archive is append-only.** Past results are never silently changed or deleted. Any correction is added as a visible, dated note, with the original kept.
- **SR-C5.** Anyone can **check that the public archive has not been rewritten.**

**D. Presence at the office**
- **SR-D1.** A computer's results count for an office **only while it is verified to be at that office.** Setups outside the office — a laptop at home, or an installation from another area — are detected and excluded.
- **SR-D2.** Presence checks must resist **simple tricks**, such as routing a home connection to look like the office.
- **SR-D3.** Presence is verified **without collecting or publishing precise locations** (see PR4).

**E. Safety of the office computer**
- **SR-E1.** **HEIMDALL never runs commands sent to it.** It only measures and reports. No one — not even the maintainers — can use it to execute anything on an office computer.
- **SR-E2.** **Nothing can connect into a HEIMDALL computer.** HEIMDALL only reaches out; it never opens a way in.
- **SR-E3.** **Least privilege.** HEIMDALL uses no more access than measuring the connection requires.
- **SR-E4.** HEIMDALL **must not weaken** the computer's own security settings or protections.
- **SR-E5.** Anything HEIMDALL stores on the computer (for example, results waiting during an outage) contains **nothing sensitive** and cannot be altered to fake results without detection.
- **SR-E6.** HEIMDALL **uninstalls completely**, leaving nothing behind.

**F. Trusted software and updates (supply chain)**
- **SR-F1.** **Every release can be verified as genuine** before installation — by staff, by maintainers, and by anyone in the public.
- **SR-F2.** Because HEIMDALL is open source, **anyone can confirm that a published release was built from the published source code**, with nothing hidden added.
- **SR-F3.** **Updates are verified as genuine before they install.** A computer refuses any update that fails verification.
- **SR-F4.** **No downgrade.** Nobody can push an older, weaker version to computers.
- **SR-F5.** **A release requires more than one maintainer's approval.** No single person, and no single stolen account, can publish a release.
- **SR-F6.** **Third-party components are reviewed** before inclusion and monitored for known vulnerabilities.
- **SR-F7.** The keys that prove a release is genuine are **protected**, and there is a plan to replace them if they are ever exposed.

**G. The HEIMDALL service and maintainers**
- **SR-G1.** **Maintainer access requires strong, multi-step sign-in.** A password alone is never enough.
- **SR-G2.** **Least privilege for maintainers.** Approving institutions, managing computers, and publishing releases are separate permissions.
- **SR-G3.** **Every maintainer action is logged** — approvals, revocations, corrections, releases — in a record maintainers cannot erase.
- **SR-G4.** **No secrets in the public code.** The open-source repository never contains passwords, keys or other credentials; a secret that is ever exposed is treated as compromised and replaced.
- **SR-G5.** **The public view is separated** from enrolment and administration. Nothing on the public map can be used to change data.
- **SR-G6.** **Abuse limits.** Submissions, enrolment attempts and public data requests are limited, so no one can flood or scrape the service.
- **SR-G7.** **Resilience.** The public map and archive stay available under heavy load or attack, and the archive is backed up so history survives a failure.

**H. Privacy as a security property**
- **SR-H1.** **Public data can never be linked back to a person.** Per-computer data is anonymised, and anonymity must hold even for small offices with only one or two computers, where combined details could identify someone.
- **SR-H2.** **No identifying details are ever published:** no computer names, user names, hardware identifiers, network addresses or precise locations.
- **SR-H3.** **Data is kept to the minimum** the product needs, and nothing is collected "in case it is useful".

**I. Security governance**
- **SR-I1.** **A published security policy** explains how to report a vulnerability privately, how quickly reports are acknowledged, and that good-faith researchers are welcome.
- **SR-I2.** **Security fixes are prioritised** over new features. Confirmed vulnerabilities are fixed and disclosed responsibly.
- **SR-I3.** **An incident plan exists before the pilot:** how a breach, a leaked code or a compromised release is contained, how affected institutions are told, and how the public is informed.
- **SR-I4.** **The threat model in this section is reviewed** at every major release, and at least once a year.
- **SR-I5.** **Independent security review before full launch.** [PROPOSED — owner to decide: which reviewer, e.g. a qualified independent tester or Sri Lanka CERT.]

### 11.4 Attack scenarios — required outcomes

| # | Scenario | Required outcome |
|---|---|---|
| S1 | An attacker on the office network intercepts results in transit | Cannot read or alter them; any interference is detected |
| S2 | A fake "HEIMDALL service" is set up to trick computers | Computers refuse to send anything to it |
| S3 | An office code leaks online | Enrolments beyond the limit, or from unexpected places, are refused or held; the code is revoked; the leak is visible to maintainers |
| S4 | A staff member installs HEIMDALL on a personal laptop and takes it home | Off-site results are excluded from the office's record and flagged |
| S5 | Someone edits results stored on a computer during an outage | The tampering is detected; the edited results are rejected |
| S6 | Someone re-sends last month's good results | Rejected as replayed |
| S7 | A malicious update is published to a lookalike download site | Computers refuse it; staff can see it is not genuine |
| S8 | A maintainer's account is stolen | The attacker cannot publish a release alone, and every action they take is logged and reversible |
| S9 | Someone floods the service with submissions | The public map stays up; the flood is limited and flagged |
| S10 | Someone combines public data to identify a staff member at a small office | Not possible from what is published |
| S11 | Someone tries to rewrite an office's history in the archive | Not possible without a visible, dated correction; the original stays |

### 11.5 Security gates

**Before the pilot (16 September 2026)**, at minimum:
- SR-A1 to A5 (enrolment and identity)
- SR-B1 to B4 (protection in transit)
- SR-C1 and C2 (tamper-evidence, no replay)
- SR-E1 to E4 (safety of the office computer)
- SR-F1 and F3 (genuine releases and updates)
- SR-G1 and G4 (maintainer sign-in, no secrets in code)
- SR-H2 (no identifying details published)
- SR-I3 (incident plan)

Any of these not met is a **blocker**, not a known issue.

**Before full launch:** every requirement in §11.3 is met, scenarios S1–S11 are tested and passed, and the independent review (SR-I5) is complete, with its findings resolved.

## 12. Adoption and roadmap

- **Pilot:** first adoption by the Eastern Province of Sri Lanka, **pilot launch 16 September 2026**, distributed through the Chief Secretary. This is a **pilot**, not full launch.
- **After the pilot is complete and verified:** letters are sent to the **Chief Secretaries of the other provinces**, inviting them to onboard and use HEIMDALL. That extends it across the country, and recognises the Eastern Province as the province where it began.
- **Longer term:** national coverage of government institutions. As an open-source project it may be adopted beyond Sri Lanka.

## 13. Success measures [PROPOSED — owner to decide]

| Measure | Target |
|---|---|
| Pilot offices reporting continuously | [ ] |
| Offices whose computers are verified to be on site | [ ] of reporting offices |
| Public map and archive availability | [ ] % |
| Provinces onboarded after the pilot | [ ] |
| Unresolved privacy or security issues | Zero |
| Security gate requirements (§11.5) met at each stage | All |

## 14. Decisions on earlier open questions

1. **Archive retention:** the public archive is **kept in full, indefinitely**.
2. **Maintainers:** named by the owner.
3. **Computer limit per office:** equal to the number of computers the office actually has (FR5), with the aim of enrolling every one (FR6a).
