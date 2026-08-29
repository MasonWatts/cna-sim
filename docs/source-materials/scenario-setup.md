# Scenario Pre-Placement (§60–61)

Building an initial `GameState` from the Campaign Game setup (§60 forces, §61 map/construction
state). Implemented in `crates/engine/src/scenario.rs`. **Principle: only chart-backed placement is
seeded; anything depending on counter/scenario-card data absent from the machine-readable rules is
omitted (not fabricated) until that data is supplied.**

## Implemented

- **Ports (§61.6)** — `GameState::setup_campaign_ports`: instantiate every port available at the
  current Game-Turn from the Port Capacity Chart (§55.3) at its full listed Efficiency Level, then
  apply the Campaign start overrides:
  - **Benghazi** begins at **0** (§61.6 — held/damaged at start; max stays 3).
  - **Tobruk** begins **three levels down** with the San Giorgio wreck: §55.12 fixes its Efficiency
    Level at 5, §55.25 gives −3 → start **2** (captured by `PortSpec.starts_blocked` → `wreck_block`).
  - All other ports at their full listed level (`from_spec` already does this since their
    `starts_blocked` is 0).
  - **Bizerta** and any other not-yet-available port (§55.17 `available_from_gt`) is skipped until
    its Game-Turn (Bizerta = GT35).
  Test: `scenario::tests::campaign_ports_setup_61_6`.

## Source discrepancies (flagged)

- **Tobruk "at seven" (§61.6 OCR).** §61.6 reads "Tobruk (at seven—and San Giorgio is still
  there)," which conflicts with §55.12 (Efficiency Level = 5) and §55.25 (−3). The explicit rules
  win → **start = 2**; the "seven" is treated as OCR noise (commit `b6ccede`).
- **Fleet roster OCR conflict (§60.45).** §60.45 states the campaign fleet as "one battleship,
  three cruisers and seven destroyers," while the adjacent setup block lists "2 Battleships, 2
  Cruisers, 4 AA Cruisers, 6 Destroyers (Alexandria) + 1 Cruiser, 1 AA Cruiser, 2 Destroyers
  (Valetta)." **Unresolved** — needs the scenario card. No fleet roster is seeded until resolved.

## Data still needed (counter/scenario-card values)

- **Warship roster + per-type Gun/AA ratings** (Cruiser, AA Cruiser, Destroyer — only BB 6/12 is
  known). Blocked by the §60.45 roster conflict above. (See `commonwealth-fleet.md`.)
- **San Giorgio** barrage + AA counter values, and its exact Tobruk hex.
- **Malta base names + initial level split.** §60.46 gives the campaign totals — an initial
  capacity of "five SGSUs" worth spread among the fields, plus starting planes (15 Hurricane I,
  12 Swordfish, 3 Gladiator, 1 Sea Gladiator), pilots (4 twos, 6 ones), 17 AA points, construction
  begins Oct 1940 — but the **field names** (Luqa, Ta'Qali, Hal Far, …) and per-base split are on
  the Malta map, not in the text.
- **Initial SGSU / aircraft laydown** for the Axis and Commonwealth (§60 force lists + map counters).

## Available but not yet built

- **§60.5 Air-Facility hex list** — the rules text DOES enumerate every starting airfield with its
  hex (Abbassia E2132, Aboukir E3815, Alexandria E3613/3714, Almaza E2133, Amiriya E2212, Barce
  B5504, Benina A4829, Dekheila E3512, Deversoir off-map E(1833), El Adem C4507, …). Fully
  data-backed; a `setup_campaign_air_facilities` reading a transcribed hex table can be built
  without any counter data. **Next candidate step.**
- Reinforcement/replacement schedules (§60.47) — "no Replacement Points before Nov 1940," CW uses
  the printed Reinforcement Track.
- Start-of-scenario construction (§61.6): railroad to Mersa Matruh (D3714), no minefields/pipeline,
  four Level-1 fortifications allowed within two hexes of Tobruk, the Sollum–Sidi Barrani and
  Tobruk road network pre-built.
