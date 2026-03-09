# Senate Simulator — Step 1: Define a Senator

## Purpose

Before simulating the Senate, we need to define the core unit of the system: an individual senator.

This project does **not** begin by modeling the Senate as a single black box that takes in bill text and outputs pass/fail. That would hide the real mechanics of Senate behavior behind a coarse chamber-level abstraction.

Instead, the simulator begins with a more grounded premise:

> The Senate is an emergent system composed of 100 political actors operating under institutional rules.

So the first step is to define what a **senator** is in the model.

---

## How to use it

The simulator now supports both fixture-based runs and live dated snapshots. The fastest way to exercise the full pipeline is:

### 1. Build a dated snapshot

Fixture snapshot:

```bash
cargo run -q -- ingest --date 2026-03-09 --source fixtures
```

Live snapshot:

```bash
export API_KEY_DATA_GOV=...
cargo run -q -- ingest --date 2026-03-09 --source live
```

Live snapshot with optional public-signal enrichment:

```bash
cargo run -q -- ingest --date 2026-03-09 --source live --include-gdelt
```

Use `--reuse-raw` when you want to rebuild normalized artifacts and snapshots from already-fetched raw files.

### 2. Build senator features from the snapshot

```bash
cargo run -q -- features-build --date 2026-03-09
```

Inspect one senator’s historical feature record:

```bash
cargo run -q -- features-inspect --date 2026-03-09 --senator-id real_s000148
```

### 3. Build and score the historical evaluation set

Generate leakage-safe evaluation artifacts:

```bash
cargo run -q -- eval-build --date 2026-03-09
```

Run the evaluator with the feature-driven stance model:

```bash
cargo run -q -- eval-run --date 2026-03-09 --stance-mode feature
```

You can compare against the older heuristic path with `--stance-mode heuristic`.

### 4. Backtest one real legislative object

```bash
cargo run -q -- backtest --date 2026-03-09 --object-id hr144 --stance-mode feature
```

This compares the predicted next event against the first aligned consequential event after the snapshot date. If there is no later aligned event, the backtest now treats that as `NoMeaningfulMovement`.

### 5. Predict one bill end to end

```bash
cargo run -q -- predict-bill --date 2026-03-09 --object-id hr144 --stance-mode feature --steps 3
```

This runs:

- senator stance derivation
- chamber analysis
- floor-action assessment
- next-event prediction
- a short rollout

### 6. Inspect one senator/object stance

```bash
cargo run -q -- stance-inspect --date 2026-03-09 --object-id hr144 --senator-id real_s000148 --stance-mode feature
```

### 7. Inspect public-signal enrichment

```bash
cargo run -q -- signals-inspect --date 2026-03-09 --object-id hr144
cargo run -q -- signals-inspect --date 2026-03-09 --senator-id real_s000148
```

### 8. Export public dashboard artifacts

```bash
cargo run -q -- predict-export --date 2026-03-09 --tracked-bills-file tracked_bills.json --out data/public --stance-mode feature --steps 3
```

### Notes

- Live ingest requires `API_KEY_DATA_GOV`.
- GDELT enrichment is optional and best-effort.
- All ingestion, feature, evaluation, and backtest flows are snapshot-date scoped.
- Historical labeling and evaluation are designed to be future-only relative to the snapshot date.

---

## Streamlit dashboard

The public Streamlit deployment is intentionally simple:

- `streamlit_app.py` is the Streamlit Community Cloud entrypoint.
- The app reads only committed JSON artifacts under `data/public/`.
- The app does not run ingestion, feature building, or prediction live.

### Public artifact flow

1. Run the Rust batch pipeline locally or in a scheduled job.
2. Export compact dashboard artifacts with `predict-export`.
3. Commit and push `data/public/`.
4. Streamlit Community Cloud redeploys from the updated GitHub repo.

### Daily refresh script

```bash
bash scripts/daily_refresh.sh 2026-03-09
```

The script runs:

- `ingest`
- `features-build`
- `predict-export`
- `git add data/public`
- `git commit`
- `git push`

### Files the app reads

- `data/public/last_updated.json`
- `data/public/summary.json`
- `data/public/tracked_bills.json`
- `data/public/bills/<object_id>.json`

This keeps the public deployment zero-cost and avoids running Rust ingestion or live API calls inside Streamlit.

---

## Why start here?

A Senate outcome is not just a function of legislative text. It emerges from:

- each senator’s substantive policy preferences,
- each senator’s procedural tendencies,
- party and leadership pressure,
- state-level incentives,
- committee structure,
- election timing,
- public signaling,
- negotiation dynamics,
- and the behavior of other senators.

If the simulator cannot represent a senator in a structured way, then any later attempt to simulate votes, delays, amendments, cloture, or bargaining will be shallow.

This first loop therefore asks:

> What information must exist in the model for one senator to behave like a plausible political agent?

---

## Design goal

The goal of this step is **not** to build a psychologically complete digital clone of each real senator.

The goal is to define a practical, computational representation that captures the main drivers of Senate behavior well enough to support:

- historical backtesting,
- prediction of senator-level positions,
- simulation of coalition formation,
- procedural forecasts,
- and later, chamber-level outcomes.

---

## What is a senator in this simulator?

A senator is modeled as a **dynamic political agent** with both stable and changing attributes.

At a minimum, a senator must have:

1. **Identity**
2. **Structural political attributes**
3. **Issue preferences**
4. **Procedural behavior tendencies**
5. **Temporal state**
6. **Observed signals**
7. **Latent stance toward a current legislative object**

This means a senator is not just a row in a vote table. A senator is a stateful object whose behavior evolves over time and depends on context.

---

## Senator model: conceptual schema

### 1. Identity layer

These fields define who the senator is.

Examples:
- `senator_id`
- `full_name`
- `party`
- `state`
- `class` (I / II / III)
- `start_date`
- `end_date`
- `prior_roles` (governor, representative, attorney general, etc.)

This layer is mostly descriptive, but it anchors all downstream behavior.

---

### 2. Structural political layer

These are relatively slow-changing attributes that shape broad behavior.

Examples:
- `ideology_score`
- `party_loyalty_baseline`
- `bipartisanship_baseline`
- `leadership_alignment`
- `committee_assignments`
- `committee_chairs`
- `state_partisan_lean`
- `reelection_cycle`
- `electoral_vulnerability`
- `donor_exposure_profile`
- `home_state_industries`
- `regional_blocs`

This layer answers:

> What kind of senator is this, structurally?

---

### 3. Issue preference layer

These fields capture how the senator tends to react to policy content.

Examples:
- `defense_score`
- `immigration_score`
- `energy_climate_score`
- `labor_score`
- `healthcare_score`
- `judiciary_score`
- `tax_spending_score`
- `trade_score`
- `tech_privacy_score`
- `foreign_policy_score`

These can be scalar values, embeddings, distributions, or learned latent vectors.

This layer answers:

> What kinds of legislation does this senator tend to support or resist?

---

### 4. Procedural behavior layer

This is crucial. Senators do not just vote on substance; they behave procedurally.

Examples:
- `cloture_support_baseline`
- `motion_to_proceed_baseline`
- `amendment_openness`
- `uc_objection_tendency`
- `symbolic_vote_tendency`
- `leadership_deference`
- `grandstanding_tendency`
- `cross_party_procedural_flexibility`
- `attendance_reliability`

This layer answers:

> How does this senator behave inside the rules of the Senate?

A senator may oppose a bill on substance but still support debate. Another may support the underlying policy but resist cloture. Without this layer, the simulator will miss a great deal of real Senate behavior.

---

### 5. Temporal state layer

These attributes change over time and capture the senator’s current political moment.

Examples:
- `current_president_alignment`
- `current_party_pressure`
- `current_primary_risk`
- `current_general_election_risk`
- `recent_media_attention`
- `current_negotiation_flexibility`
- `current_public_rigidity`
- `current_leverage`
- `issue_salience_in_state`
- `recent_vote_streak_by_topic`

This layer answers:

> What is happening around this senator right now that may alter behavior?

---

### 6. Observed signal layer

Not all behavior is formal Senate action. Public statements matter.

Examples:
- press releases
- floor speeches
- cable/news interviews
- social posts
- sponsor/cosponsor decisions
- public endorsements/oppositions
- interviews from local press
- caucus letters
- whip rumors or reported positioning

These are not the same as votes. They are **observations** that help estimate latent stance.

This layer answers:

> What signals has the senator recently emitted about this issue or coalition?

---

### 7. Latent stance layer

This is the core prediction layer for a senator with respect to a particular bill, amendment, motion, or procedural move.

For a current legislative object, a senator should have estimated values like:

- `substantive_support`
- `procedural_support`
- `public_support`
- `negotiability`
- `confidence`
- `defection_probability`
- `absence_probability`

This is the live internal state the simulator will update when new legislation, amendments, statements, negotiations, or leadership moves occur.

This layer answers:

> Where is this senator currently, on this specific question?

---

## Key modeling principle

A senator should **not** be treated as a static label generator.

The model should assume:

- some attributes are stable,
- some drift slowly,
- some update rapidly,
- and some are only partially observable.

So each senator is best thought of as a **stateful agent under uncertainty**.

---

## Minimum viable senator definition

For the first implementation, keep it tight.

A useful v1 senator object likely needs only:

### Static fields
- identity
- party
- state
- ideology
- committee assignments
- reelection timing
- broad issue preference vector
- broad procedural tendency vector

### Dynamic fields
- current public position
- current substantive support estimate
- current procedural support estimate
- current negotiability
- current leadership alignment
- current electoral pressure

That is enough to begin building a serious simulator without drowning in feature creep.

---

## What this step is trying to avoid

This step is explicitly avoiding several bad starting points.

### Bad starting point 1: pass/fail only
A chamber-level pass/fail model may achieve superficial accuracy while teaching us almost nothing about Senate behavior.

### Bad starting point 2: vote table only
Historical votes are essential, but a senator is more than a history of yes/no outcomes.

### Bad starting point 3: text-only bill classifier
Bill text matters, but senators react to more than text: timing, actors, procedure, coalition signals, and electoral constraints all matter.

### Bad starting point 4: purely independent senators
Each senator must be individually modeled, but not as an isolated predictor. Senator behavior is interdependent.

---

## Initial object sketch

A senator object might look conceptually like this:

```json
{
  "senator_id": "sen_001",
  "full_name": "Example Senator",
  "party": "D",
  "state": "CA",
  "class": "I",
  "in_office": {
    "start": "2017-01-03",
    "end": null
  },
  "structural": {
    "ideology_score": -0.62,
    "party_loyalty_baseline": 0.91,
    "bipartisanship_baseline": 0.24,
    "committee_assignments": ["Judiciary", "Budget"],
    "reelection_year": 2028,
    "electoral_vulnerability": 0.18
  },
  "issue_preferences": {
    "defense": 0.35,
    "immigration": -0.55,
    "energy_climate": -0.78,
    "labor": -0.64,
    "healthcare": -0.71,
    "tax_spending": -0.12
  },
  "procedural": {
    "cloture_support_baseline": 0.72,
    "motion_to_proceed_baseline": 0.81,
    "uc_objection_tendency": 0.10,
    "leadership_deference": 0.74,
    "amendment_openness": 0.66
  },
  "dynamic_state": {
    "current_public_position": "undeclared",
    "current_substantive_support": 0.58,
    "current_procedural_support": 0.64,
    "current_negotiability": 0.47,
    "current_party_pressure": 0.61,
    "current_issue_salience_in_state": 0.32
  }
}

This is only illustrative. The exact schema can change.
