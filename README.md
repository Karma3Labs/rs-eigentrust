# rs-eigentrust - Trust Computer for Snaps

We perform [EigenTrust](https://nlp.stanford.edu/pubs/eigentrust.pdf) algorithm
on peer-to-peer trust signals.
These signals include both trust and distrust credentials, conforming to this
**[draft CAIP](https://github.com/dayksx/CAIPs/blob/main/CAIPs/caip-x.md).**
The results of the compute provide reputation score for users in two contexts -
Software Security and Software Development. These User reputation scores are
used to calculate Snap scores and Community Sentiment, which helps surface Snaps
considered Safe or Malicious based on community reputation.

## Running simulated test

Ensure that `jq` is installed on the system, then run `./run_simulation.sh` with
two arguments:

- Verifiable credentials filename.
- Pre-trust filename

Example:

```sh
./run_simulation.sh ~/testcase.csv ~/pretrust.txt
```

It builds/installs everything on the local machine, then runs a local pipeline
with the given input. In another terminal window, check the contents of
`ssc.log`; once it prints the `starting run` message 2-3 times in a row, stop
the pipeline using Ctrl-C.

The output is found in the subdirectories of `./spd_scores/` (created as
needed):

* `./spd_scores/1/` for the software development scope
* `./spd_scores/2/` for the software security scope

Each directory contains a series of archive (`.zip`) + manifest (`.json`) pairs,
named after the issuance UNIX timestamp in milliseconds.

### Simulation Input

#### Verifiable Credentials File

Verifiable credentials are fed into the system using a CSV file.
Each record should have four fields (in this order):

1. `id`: A monotonically increasing sequence number, from 1
2. `timestamp`: A UNIX timestamp in millisecond precision.
3. `schema_id`: `1` if the VC is a `ReviewCredential`, `2` if
   a `TrustCredential`
4. `schema_value`: A (CSV-quoted) verifiable credential JSON, in the format
   documented in the
   **[draft CAIP](https://github.com/dayksx/CAIPs/blob/main/CAIPs/caip-x.md).**

   To ease processing, the following VC fields are not used and may be left
   empty:

    - `proof`
    - `credentialSubject.trustworthiness[].reason` (in `TrustCredential`-s)
    - `credentialSubject.statusReason` (in `ReviewCredential`-s)

Sample input CSV file:

```csv
id;timestamp;schema_id;schema_value
1;1707490806644;2;"{""@context"":[""https://www.w3.org/2018/credentials/v2""],""credentialSubject"":{""id"":""did:pkh:eip155:59144:0xefc6191B3245df60B209Ec58631c7dCF04137329"",""trustworthiness"":[{""level"":1,""reason"":[],""scope"":""Software development""},{""level"":0,""reason"":[],""scope"":""Software security""}]},""issuanceDate"":""2024-02-09T10:22:21.670Z"",""issuer"":""did:pkh:eip155:59144:0x6eCfD8252C19aC2Bf4bd1cBdc026C001C93E179D"",""proof"":{},""type"":[""VerifiableCredential"",""TrustCredential""]}"
2;1707493457525;2;"{""@context"":[""https://www.w3.org/2018/credentials/v2""],""credentialSubject"":{""id"":""did:pkh:eip155:59144:0xefc6191B3245df60B209Ec58631c7dCF04137329"",""trustworthiness"":[{""level"":1,""reason"":[],""scope"":""Software development""},{""level"":0,""reason"":[],""scope"":""Software security""}]},""issuanceDate"":""2024-02-09T10:22:21.670Z"",""issuer"":""did:pkh:eip155:59144:0x6eCfD8252C19aC2Bf4bd1cBdc026C001C93E179D"",""proof"":{},""type"":[""VerifiableCredential"",""TrustCredential""]}"
3;1707990795781;1;"{""@context"":[""https://www.w3.org/2018/credentials/v2""],""credentialSubject"":{""currentStatus"":""Endorsed"",""id"":""snap://lfxsHs6C6buodVo0zVJakNAHgIXAGkHyVTL12Rw0xdw="",""statusReason"":{""type"":""Endorse"",""value"":[]}},""issuanceDate"":""2024-02-15T09:53:10.811Z"",""issuer"":""did:pkh:eip155:59144:0x3892967AA898d7EeBf1B08d3E1F31B2F4C84317A"",""proof"":{},""type"":[""VerifiableCredential"",""ReviewCredential""]}"
4;1707990982793;1;"{""@context"":[""https://www.w3.org/2018/credentials/v2""],""credentialSubject"":{""currentStatus"":""Disputed"",""id"":""snap://lfxsHs6C6buodVo0zVJakNAHgIXAGkHyVTL12Rw0xdw="",""statusReason"":{""type"":""Malicious"",""value"":[]}},""issuanceDate"":""2024-02-15T09:56:19.055Z"",""issuer"":""did:pkh:eip155:59140:0x6eCfD8252C19aC2Bf4bd1cBdc026C001C93E179D"",""proof"":{},""type"":[""VerifiableCredential"",""ReviewCredential""]}"
```

Additional considerations:

* The `credentialSubject.trustworthiness[].scope` in a `TrustCredential` can be
  one of:
    * `Honesty`
    * `Software security`
    * `Software development`
* The `Software security` and `Software development` scopes are currently only
  used for expressing trust, not distrust.
* Similarly, the `Honesty` scope is currently only used for expressing distrust,
  not trust.
* The action of withdrawing a previously issued `TrustCredential` for a
  peer/scope is done by re-issuing another `TrustCredential` for the same
  peer/scope with `credentialSubject.trustworthiness[].level` of `0`.
* A peer can signal a change in their opinion about a Snap version by re-issuing
  another `ReviewCredential` for the same Snap version with the opposite
  `currentStatus` field value (flip between `Endorsed` and `Disputed`).
* Currently it is not possible to withdraw a previously issued
  `ReviewCredential`, i.e. to go back to the "no opinion" state.
    * This will later be designed/implemented, e.g. using mechanisms such as
      [VC Revocation List](https://w3c-ccg.github.io/vc-status-rl-2020/) and/or
      [VC Ethr Revocation](https://spherity.github.io/vc-ethr-revocation-registry/).

#### Pre-trust file

Pre-trust is configured into the system using a text file with two fields,
separated by a single space:

- Peer DID
- Relative weight (use `1` for everyone for equal distribution).

Sample file with 5 equally pre-trusted peers:

```
did:pkh:eip155:1:0x44dc4e3309b80ef7abf41c7d0a68f0337a88f044 1
did:pkh:eip155:1:0x4EBee6bA2771C19aDf9AF348985bCf06d3270d42 1
did:pkh:eip155:1:0xE5aF1B8619E3FbD91aFDFB710b0cF688Ce1a4fCF 1
did:pkh:eip155:1:0x224b11F0747c7688a10aCC15F785354aA6493ED6 1
did:pkh:eip155:1:0x690FCDE0B69B8B66342Ac390A82092845c6F7f1c 1
```

### Simulation Output File

All output `.zip` archive files belong to the same series, identified by the
"epoch" timestamp (the pipeline's start timestamp). Each `.zip` archive file is
a score snapshot of every user and Snap known to the system, as of a specific
"effective" anchor time. The file contains:

* `MANIFEST.json` – the archive manifest
* `peer_scores.jsonl` and `snap_scores.jsonl` – peer/Snap score VCs, one per
  line.

Sample manifest `spd_scores/2/1710239744000.json` (pretty-printed):

```json
{
  "effectiveDate": "2024-02-23T17:12:00.000Z",
  "epoch": "2024-03-12T10:35:44.000Z",
  "issuanceDate": "2024-03-12T10:35:44.124Z",
  "issuer": "did:pkh:eip155:1:0x23d86aa31d4198a78baa98e49bb2da52cd15c6f0",
  "locations": [],
  "proof": {},
  "scope": "SoftwareSecurity"
}
```

The manifest above states that:

* The `.zip` file containing this manifest belongs to the series (`epoch`) that
  started on 2024-03-12 10:35:44 UTC.
* The scores found within have been calculated after processing all input VCs
  registered before 2024-02-23 17:12:00 UTC.
* The snapshot was issued at 2024-03-12 10:35:44.124 UTC.
* The scores found within are in the software security scope.

## Algorithm Description

### **Inputs**

Users can issue explicit trust or distrust attestations to each other and Snaps.
The following attestations are used for this prototype:

***User to User attestations***

- I assert that I trust ('Endorse') an account/user's software security
  (auditor) abilities
- I assert that I distrust ('Report') an account/user's software security
  (auditor) abilities
- I assert that I trust ('Endorse') an account/user's software development
  (developer) abilities
- I assert that I distrust ('Report') an account/user's software development
  (developer) abilities

***User to Snap attestations***

- I dispute a snap version
- I endorse a snap version

### **System Architecture**

The system performs a few tasks in sequence:

- Take the input data (attestations) from the data layer (Indexer)
- Transform the data for pre-processing (Attestations Transformer and Linear
  Combiner)
- Perform core compute and avail the compute result (Trust Computer)
- Perform post-processing on computed results (Snaps Score Logic)

![KKA7fSm1nGwen6Gaot0f2Zu1dU.jpg.webp](images/KKA7fSm1nGwen6Gaot0f2Zu1dU.jpg.webp)

### How does the Algorithm work?

**Phase 1: Determine Trustworthiness of Users**

We model a graph (network) of users and snaps, then run the EigenTrust algorithm
to compute trustworthiness of each user based on the attestations ('Trust
Credentials') received from others.

**1a: EigenTrust scores for Users**

This phase takes the trust graph expressed between users as the input, then
outputs trust scores to each of them. Only positive P2P trust is considered;
negative trust is applied in phase 1b.

**1b: Distrust Adjustment for Users**

In general, distrust signals cannot be recursively interpreted, so we do not use
the distrust signals as part of local trust in Phase 1a. Instead, once Phase 1a
is finished and we have trust score for all users, we apply a one-shot discount
of trust score, with using distrust opinions by auditors. The distrust opinions
held by the same user are normalized to their trust score, that is, a user is
allowed to discredit/discount other users as much as his own trust standing,
e.g. if a user X distrusts 7 other users, each of the 7 users’ trust score will
receive a deduction equal to 1/7 of X’s trust score. Distrust opinions of only
those who have a positive standing count; if someone received zero score in
Phase 1a, their distrust opinions won’t matter.

**Output:** Two scores for each EOA, security trust score (auditor) and dev
trust score (developer). It's a number between -1.0 and +1.0.

**Phase 2: Determine Community Sentiment of Snaps**

Once Phase 1 is finished, each user gets assigned a trust score, which is used
to weight the review of that user about a Snap.

The Snaps’s **security score** is calculated as a weighted average of the
individual attestations from peers about the Snap being secure or insecure.
Given an individual rating $R(s, p)$ (0 or 1) for a Snap $s$ by a peer $p$ and
the trust score of the peer $T(p)$, the Snap’s overall security rating is given
as:

$$
R_c(s) = {{\sum R(s,p)T(p)} \over {\sum T(p)}}
$$

A Snap also gets a **snap confidence score**. $C(s)=\sum T(p)$ is defined as the
cumulative trust **confidence level** of the resulting score. It helps factor in
the reputation of users who have endorsed or reported a Snap, and is useful in
fending of a class of sybil attacks.

**Output:** Each snap will get only one score (security), which consists of two
numbers: Snap security score (0.0-1.0) and Score confidence (0.0-1.0).

### The Scoring Thresholds for Community Sentiment

This is a post-processing step. It basically enables any developer to utilize
the user reputation scores to create their own Safety thresholds for Snaps.
These thresholds can then power ranking, recommendation on any Snap Directory or
Marketplace.

For this prototype, we have used conservative thresholds for calculating
Community Sentiment for User and Snap reputation. The detailed explanation of
the Community Sentiment logic is below. **Anyone can run the compute steps
described above on their local machine and generate these scores to verify that
the compute was done correctly.**

$P$ denotes the set of all peers (security experts) in the network. For a peer
$p$, $T(p) \in [-1..1]$ denotes the trust standing (distrust-adjusted EigenTrust
score) of the peer (in the “security” scope), and $T^+(p) \in [0..1]$ denotes
the positive-local-trust-only trust standing (pure EigenTrust score without
distrust adjustment) of the peer.

We appoint some peers (auditors) so that their opinion immediately matters
(precise definition is given below). We call them *highly trusted auditors.*
[Note – Under the current trust graph model, we define highly trusted auditors
as peers directly endorsed by the pre-trusted peers. – end note]
$P_h \subset P$ denotes the set of all highly trusted auditors in the network.

Given a Snap $s$, $O(s) \subset P$ is the set of peers who opined (filed a
ReviewCredential) on $s$. For $p \in O(s)$, $R(s,p) \in [0..1]$
denotes the peer $p$’s status opinion about the Snap $s$.

We define the security score for the Snap $s$ as a set of two numbers:

- **Score value** $R_c(s) \in [0..1]$
- **Score confidence level** $C(s) \in [0..1]$

The score value is the weighted average of opinions, weighted by the opiner’s
trust standing; the score value is the sum of all opiners’ trust standings:

$$
\begin{align*}C(s) &= \sum_{p \in O(s)} T(p)\\
R_c(s) &= {{\sum_p R(s,p)T(p)} \over {\sum_p T(p)}} \\
&= {{\sum_p R(s,p)T(p)}
\over C(s)} \end{align*}
$$

Until a Snap $s$ gathers strong enough of collective opinions, as measured by
the opiners’ trust standings $C(s)$, we do not display the community sentiment.
A Snap in this state is called **Insufficient Reviews.**  The collective opinion
$R_c(s)$ does not matter in this case, e.g. it may solely consist of malicious
sybils’ opinions.

Once $s$’s collective opinion becomes strong enough, i.e. $C(s)$ reaches a
threshold, we take a look at the actual collective opinion $R_c(s)$. The
threshold is set in such a way that any highly trusted auditor opinion is
sufficient, i.e. $C(s) \ge T^+(d)$, where $d$ is the weakest highly trusted
auditor (weakest = with lowest positive-LT-only trust score).

[Note – We use positive-LT-only trust score as the threshold criteria
to keep the bar high. If we used negative-adjusted trust score,
the bar could be brought arbitrarily low if a highly trusted auditor became
targeted by other highly trusted peers with distrust credentials. – end note]

We consider $R_c(s)$ by comparing it against two thresholds $R_E$ and $R_R$ (
$0 < R_R < R_E < 1$):

- Iff $R_c(s) > R_E$, we label $s$ with the **Endorsed by Community** badge (
  “Endorsed” hereafter).
- Iff $R_c(s) < R_R$, we label $s$ with the **Reported by Community** badge (
  “Reported” hereafter).
- Otherwise, i.e. $R_R \le R_c(s) \le R_E$, we label $s$ with the **In Review**
  badge.

We define $R_E$ and $R_R$ **conservatively**, such that a Snap cannot be in the
Endorsed or Reported state – and instead fall into the In Review state – if at
least one highly trusted auditor disagrees with that disposition:

- As long as at least one highly trusted auditor reports a Snap as insecure, the
  Snap cannot be in the Endorsed state.
- As long as at least one highly trusted auditor endorses a Snap as secure, the
  Snap cannot be in the Reported state.

In other words, in order for a Snap to be labeled with the Endorsed or Reported
badge, the highly trusted auditors who opined on the Snap must all agree on the
disposition – they must be *unanimous*.

For the conditions above, we consider the worst case, where everyone with
positive trust standing – not just highly trusted auditors – has voiced opinion
about $s$, that is, $C(s)$ cannot be any higher, and only one highly trusted
auditor $d \in P_h$ (for “dissident”) disagrees with everyone else about $s$,
where $d$’s positive-LT-only trust standing is the lowest among all highly
trusted auditors (that is, $d$ is the “weakest dissident”).

- If $d$ is the only one that reported the Snap whereas everyone else endorsed
  it:

  $$R(s,p) = 1\text{ if }p \ne d\text{, }0\text{ if }p = d$$

  This results in the highest Snap score value threshold where the Snap is still
  “In Review” state, due to the sole dissident. That is:

  $$R_E = {C(s)-T^+(d) \over {C(s)}}=1-{T^+(d) \over C(s)}$$

- Similarly, if $d$ is the only one that endorsed the Snap whereas everyone else
  reported it:

  $$R_R = {T^+(d) \over {C(s)}}$$

All in all, the Snap $s$ earns:

- $C(s) < T^+(d)$ ⇒ **Insufficient Reviews**
- $C(s) \ge T^+(d)$:
    - $R_c(s) > 1-{T^+(d) \over C(s)}$ ⇒ **Endorsed**
    - $R_c(s) < {{T^+(d)} \over C(s)}$ ⇒ **Reported**
    - ${{T^+(d)} \over C(s)} \le R_c(s) \le {1-{T^+(d) \over C(s)}}$ ⇒ **In
      Review**

### **Community Sentiment Status/Badges**

**Snaps Community Sentiment Badges:**

- **[Insufficient Reviews]:** A Snap which doesn't have enough reviews from
  highly reputable users ('highly trusted' threshold defined above).
- **[Endorsed]:** A Snap which has received endorsements (and no reports) from
  highly reputable Users.
- **[In Review]:** A Snap which has received at least 1 report from a highly
  reputable auditor *and* at least 1 endorsement from a highly reputable auditor
  will be in the status until resolved.
- **[Reported]:** A Snap which is Reported by reputable auditors

**User community sentiment badges:**

- **[Highly Trusted]:** A User who has received endorsements from other highly
  trusted users
- **[Reported]:** A User who has been Reported by a highly trusted User
