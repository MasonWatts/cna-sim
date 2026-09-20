import type { InfoPanelData } from "./lore";

// Rules info for the naval panels (§55/§56). The Ports box splits into three tooltip zones: the box
// title + port names (general), the Shipping Tonnage columns, and the Troop Transfer columns.
export const PORT_SYSTEM_INFO: InfoPanelData = {
  title: "Ports (§55)",
  rules: {
    description:
      "Each port has an Efficiency Level, shown current/max (e.g. 3/3). Efficiency is knocked down by bombing, air-laid harbour mines (§55.27), or a scuttled wreck blocking the harbour (§55.25) — a held-down port shows ⚠ and is restored or cleared by engineers over time. Its efficiency governs both the supply tonnage it can land (Shipping Tonnage) and, alongside its Stacking-Point capacity, the troops it can move by sea (Troop Transfer).",
  },
};

export const PORT_TONNAGE_INFO: InfoPanelData = {
  title: "Port Shipping Tonnage (§55.3)",
  rules: {
    description:
      "The supply tonnage a port can land per Operations Stage, shown as throughput/base (e.g. 500/500t). The base is the capacity at full efficiency; the current throughput scales with the Efficiency Level (base × efficiency ÷ max), so 1/3 efficiency yields a third of the base tonnage and 0/N is inoperable. Convoys (§56) deliver into this capacity.",
  },
};

export const PORT_TRANSFER_INFO: InfoPanelData = {
  title: "Port Troop Transfer (§30.5/§55.13)",
  rules: {
    description:
      "The ⇅ figures are the port's personnel capacity in Stacking Points that may enter / leave per Operations Stage (§55.13) — governing how many troops the Commonwealth can move by sea through it; a minor port (0 in) can ship men out but not receive them. Only one naval troop transfer may use a port per Operations Stage (§30.5); the checkbox at right is ticked once this port has transferred this stage.",
  },
};

export const CONVOY_SYSTEM_INFO: InfoPanelData = {
  title: "Axis Convoys (§56)",
  rules: {
    description:
      "Axis reinforcements and supply arrive by scheduled Naval Convoys (§56): each is planned to a destination port with an arrival Game-Turn (listed as GT, lane, destination, and tonnage), and on arrival its surviving tonnage is landed there as supply.",
  },
};

export const OFF_MAP_INFO: InfoPanelData = {
  title: "Off-Map: Tripoli/Tunisia (§8.8)",
  rules: {
    description:
      "An Axis-only chain of off-board boxes — Tunis, Gabes, Tripoli, Tripolitania — where seaborne reinforcements and supply arrive at the ports of Bizerta (Tunis) and Tripoli. Units are not on the map yet: they march one Operations Stage at a time (spending their full CPA, §8.84) through the boxes toward the map, entering the board at the Nofilia junction (hex A2802). The boxes have unlimited stacking, water, and supply, and no breakdown checks.",
  },
};

export const OFF_MAP_AA_INFO: InfoPanelData = {
  title: "Off-Map Box AA (§8.8)",
  rules: {
    description:
      "Each Tripoli/Tunisia box has an intrinsic anti-aircraft value that fires at Commonwealth planes flying missions over the box (Malta- or Africa-based aircraft that can reach it). A higher value better protects units resting or transiting there — Tripoli (26) is the strongest, then Tunis (15), with Tripolitania and Gabes at 10.",
  },
};

export const OFF_MAP_PORT_INFO: InfoPanelData = {
  title: "Off-Map Debarkation Port (§8.8)",
  rules: {
    description:
      "The box's seaborne point of entry: Axis naval convoys (§56) land supply and reinforcements here — Tripoli (port of Tripoli) and Tunis (port of Bizerta). Units arriving by sea start in that box, then march the chain toward Nofilia (§8.86 limits moving them the turn they arrive). Portless boxes (Tripolitania, Gabes) can be reached only overland.",
  },
};

export const OFF_MAP_TRANSIT_INFO: InfoPanelData = {
  title: "Off-Map Movement Tracks (§8.84/§8.89)",
  rules: {
    description:
      "Units march one leg at a time between boxes, spending their full movement allowance each Operations Stage (§8.84). The stages a leg takes depend on the unit's CPA (§8.89): a fast unit (CPA ≥25) covers a leg in 1 stage; a slow unit (CPA <15) takes 3 (or 5 on the Tunis→Gabes leg). Each cell here is one Operations Stage of the leg; a unit sits on the cell matching its progress. Movement is normally toward the map (Nofilia), but a unit may reverse and head back inland (§8.85).",
  },
};

export const MED_BASE_INFO: InfoPanelData = {
  title: "Mediterranean Air Bases (§43)",
  rules: {
    description:
      "The Axis must base a required share of its heavy bombers (He 111 / Ju 88 / FW 220) off-map in the Italy/Sicily and Crete boxes (§43.1): early war they sit in Italy/Sicily, and from mid-1941 the bulk shift to Crete. From there they bomb Malta, fly CAP for and bomb convoys, and strike North Africa within range; they are auto-supplied (§43.21) and transfer between boxes with a one-stage transit delay (§43.14). No German bombers arrive until the Luftwaffe joins the DAK (GT26+), so these boxes are empty early in the campaign.",
  },
};

export const CW_OFF_MAP_INFO: InfoPanelData = {
  title: "Commonwealth Off-Map Air (§36.53)",
  rules: {
    description:
      "The Allied off-map air facilities east of the map (§36.53): six Canal-zone boxes — Port Said (a flying-boat basin, capacity 3), Abu Seier, Ismailia, Fayid, Deversoir, and Kabrit (capacity 9 each) — plus Ethiopia (unlimited, East African, not available until Game-Turn 35). Commonwealth squadrons base here within each field's squadron capacity; a squadron's mission range is the chart distance from the box to its on-map entry hex plus the hex distance from there to the target (§37.4). The six Canal-zone boxes are the \"North African\" off-map facilities used by the §34.82C SGSU-creation test; Ethiopia is excluded.",
  },
};

// §36 air-facility kinds — a short rules blurb keyed off AirFacilityView.kind, shown when hovering
// a facility's type glyph.
export const FACILITY_KIND_INFO: Record<string, InfoPanelData> = {
  Airfield: {
    title: "Airfield (§36.1)",
    rules: { description: "A land airfield: bases fighters, bombers, and transports (not flying-boats, §36.3). Each capacity level services planes; on Malta a level holds up to 18 planes (§44.14). Reducible by bombing, rebuilt level by level (§36.12)." },
  },
  AirLandingStrip: {
    title: "Air Landing Strip (§36.2)",
    rules: { description: "A minimal land strip for normal aircraft (not flying-boats). Small capacity; built or rebuilt from scratch in one construction segment (§24.74)." },
  },
  FlyingBoatBasin: {
    title: "Flying Boat Basin (§36.3)",
    rules: { description: "An “airfield” for flying-boats only (§36.3) — normal aircraft may not use it. Capacity three squadrons; one intrinsic AA point. Built only in coastal hexes by Engineer battalions (§24.73)." },
  },
  FlyingBoatAlightingArea: {
    title: "Flying Boat Alighting Area (§36.4)",
    rules: { description: "Like a basin but capacity one squadron, and immune to artillery barrage (still vulnerable to air bombardment). Quick to build (one segment) by engineers or an SGSU (§24.72)." },
  },
};
export const facilityInfo = (kind: string): InfoPanelData => FACILITY_KIND_INFO[kind] ?? FACILITY_KIND_INFO.Airfield;

// Island-wide Malta generalities (the box title). Per-field specifics live on the child elements:
// the kind glyph (FACILITY_KIND_INFO), the capacity readout (MALTA_CAPACITY_INFO), and the AA
// readout (MALTA_AA_INFO).
export const MALTA_INFO: InfoPanelData = {
  title: "Malta (§44)",
  rules: {
    description:
      "The British island base off Sicily. No land combat — the Axis may not invade it (§44.3); the only threat is Axis air raids (§44.2). Its air facilities host RAF planes that interdict Axis convoys, and Valetta is also a naval base for Commonwealth ships (§30.18). The facilities are permanent and printed on the map — never destroyed, only reduced to zero effectiveness by bombing (§44.12). Planes may transfer freely between the Malta fields (§44.15) and to/from the African fields within range (§44.17). Hover a field's type glyph, capacity, or AA figure for its own rules.",
  },
};

export const MALTA_CAPACITY_INFO: InfoPanelData = {
  title: "Malta plane capacity (§44.14)",
  rules: {
    description:
      "Malta uses no SGSUs. Each capacity level of a field handles up to 18 planes of any type (so a 6-level airfield holds 108); the readout shows ✈ planes-based / capacity. Planes are grouped as if squadrons (§44.14) and are automatically refuelled and rearmed each stage, but must still be refit like any other planes (§44.16). Bombing reduces a field's levels — and thus its capacity — toward zero (§44.12); the Commonwealth rebuilds levels with the Malta Air Facility Construction Table (§44.13).",
  },
};

export const MALTA_AA_INFO: InfoPanelData = {
  title: "Malta AA / Flak (§44.18)",
  rules: {
    description:
      "AA is diverted from the Commonwealth replacement pool — one Light (+1) or Heavy (+4) point per month, capped at 48 island-wide (§44.18). Points are held per-field and may be freely redistributed between fields like planes (§44.15); a raid draws only the raided field's flak (§44.24). Escorting fighters can suppress Light flak (one point per three, §40.74) but never Heavy — the Heavy portion is shown as (nH). Each field also has one intrinsic AA point (§36.18, the “+ 1”), but it fires only against strafing/dive-bombing, not the level-bombing Malta raid.",
  },
};
