import type { InfoPanelData } from "./lore";

// Overview shown on the "Terrain Key" heading itself.
export const TERRAIN_KEY_INFO: InfoPanelData = {
  title: "The Map (§8)",
  rules: {
    description:
      "The board is a hex grid — each hex is roughly 8 km across, and one Game-Turn is one week of real time. Every hex has a terrain type (the swatches below) that sets its movement cost, vehicle-breakdown risk, stacking limit, and combat-column effects; the shared edges between hexes carry their own features (rivers, ridges, escarpments, slopes, wadis) and routes (roads, tracks, railroads) that modify crossing. Hover any swatch for its specific effects.",
  },
};

// Rules info for Terrain-Key entries, keyed by the legend's terrain / route / edge keys. Rules text
// is grounded in the Terrain Effects Chart (data/reference/terrain.ron, §8.37) and §8.4x / §15.3x;
// combat "shifts" move columns in the named player's favor. Historical LORE for these keys is NOT
// here — it lives in the runtime data file web/public/lore/terrain.json (fetched via
// loadLore("terrain")) and is merged onto these entries at hover time, so a content author edits
// JSON with no rebuild (same contract as places.json).
export const TERRAIN_INFO: Record<string, InfoPanelData> = {
  // ---- Terrain ----
  Clear: {
    title: "Clear",
    rules: { description: "Open going. Move in for 2 CP (foot or motorized). No combat-column shifts; stacking limit 6. Lowest desert breakdown risk after the delta/cities." },
  },
  SandGravel: {
    title: "Rock / Gravel",
    rules: { description: "Firm rocky/gravel plain. Move in for 2 CP. No combat shifts; stacking 6. Rougher footing than Clear means a somewhat higher vehicle breakdown risk." },
  },
  SaltMarsh: {
    title: "Salt Marsh",
    rules: {
      description:
        "§8.44: treacherous salt sand. Except Light Trucks, Recce-type units, and motorcycle infantry, vehicles may enter or leave only along a Road or Track; a prohibited vehicle that enters off-track is Abandoned (§5.33), and no motorized unit or AFV may assault a defender here. Entry costs 3 CP foot / 2 CP motorized. A unit caught defending in a Salt Marsh is bogged — Close Assault shifts one column in the ATTACKER's favor (§15.32); barrage and anti-armor are unaffected.",
    },
  },
  HeavyVegetation: {
    title: "Heavy Vegetation",
    rules: { description: "Dense growth. Move in for 3 CP. Cover aids the defender: anti-armor and close assault each shift one column in the defender's favor; barrage unaffected. Stacking 6." },
  },
  Rough: {
    title: "Rough",
    rules: { description: "Broken ground. Move in for 3 CP foot / 4 CP motorized. Defender-favorable: barrage −1, anti-armor −1, close assault −2 columns. Stacking 6; elevated breakdown risk." },
  },
  Mountain: {
    title: "Mountain",
    rules: { description: "Move in for 4 CP foot / 6 CP motorized. Strongly defensive: barrage & anti-armor −2, close assault −3 columns in the defender's favor. Stacking only 3; very high vehicle breakdown." },
  },
  Delta: {
    title: "Delta",
    rules: { description: "Cultivated Nile delta. Move in for 2 CP foot / 4 CP motorized. No combat shifts; stacking 6. The lowest breakdown risk on the map." },
  },
  Desert: {
    title: "Desert",
    rules: { description: "Deep, soft desert. Move in for 3 CP foot / 4 CP motorized. No combat shifts; stacking 6 — but the harshest terrain for vehicles, with an extreme breakdown risk (§21)." },
  },
  MajorCity: {
    title: "Major City",
    rules: { description: "Move in for 1 CP foot / ½ CP motorized; stacking 8. A city's combat defense comes from its built-in fortification (§25): Alexandria and Cairo are Level 3, other major cities Level 2." },
  },
  Swamp: {
    title: "Swamp",
    rules: { description: "May be entered or left only along a Road or Railroad; otherwise impassable to all units." },
  },
  Sea: {
    title: "Sea",
    rules: { description: "Open water — impassable to land units; only naval movement applies (§30/§56)." },
  },

  // ---- Point features ----
  Port: {
    title: "Port",
    rules: { description: "Where seaborne supply and reinforcements land (§55/§56). Throughput each stage is the port's Efficiency Level × its tonnage; efficiency is knocked down by bombing, harbour mines (§55.27), or a scuttled wreck blocking the harbour (§55.25 — San Giorgio at Tobruk), and is regained or cleared by engineers." },
  },
  Village: {
    title: "Village / Bir",
    rules: { description: "A village or bir (well) holds a limited water source for units in the hex (§52.16). Unlike major-city and oasis wells, these wells can be poisoned (any unit: 1 CP, a die roll of 1 salts it) or run dry. Villages are not fortifications (§25.12)." },
  },
  Oasis: {
    title: "Oasis",
    rules: { description: "A natural supply dump for Water and Stores/food (§52.3): its wells never deplete and can't be poisoned, and a unit sitting in an oasis has all the water and Stores it needs for the whole game. Pipelines may not be built from an oasis." },
  },
  CwTrainingArea: {
    title: "Training Area (Commonwealth)",
    rules: { description: "A designated Commonwealth Training Area (§20.43). Units may Train here (§17.3), and Replacement-Point pools count down their Training only while sitting in a friendly training area before they can be absorbed." },
  },
  AxisTrainingArea: {
    title: "Training Area (Axis)",
    rules: { description: "A designated Axis Training Area (§20.43 — Tripoli / any major city). Units may Train here (§17.3), and Replacement-Point pools count down their Training only while sitting in a friendly training area before they can be absorbed." },
  },

  // ---- Routes (a hexside overlay; the along-route cost replaces the hex's terrain cost) ----
  Road: {
    title: "Road",
    rules: { description: "Move along it for ½ CP motorized / 1 CP foot, replacing the hex's terrain cost. No combat effect." },
  },
  UnfinishedRoad: {
    title: "Unfinished Road",
    rules: { description: "A road under construction — no movement benefit until it is completed (§23)." },
  },
  Track: {
    title: "Track",
    rules: { description: "A desert track: move along it for 1 CP, replacing terrain cost. A track is also the only way most vehicles may cross a Salt Marsh (§8.44)." },
  },
  Railroad: {
    title: "Railroad",
    rules: { description: "Built railroad: rail movement (§8.7) costs 0 CP along connected track. For the Commonwealth an operating rail hex also counts as an unlimited water source (§52.22)." },
  },
  UnfinishedRailroad: {
    title: "Unfinished Railroad",
    rules: { description: "Unbuilt track — inert until completed (§24.67): no rail movement and no water function." },
  },

  // ---- Edge features (crossing a shared hexside) ----
  Wadi: {
    title: "Wadi",
    rules: { description: "Cross for 1 CP foot / 4 CP motorized. A Close Assault made across a wadi shifts one column in the defender's favor. Does not block ZOC." },
  },
  Ridge: {
    title: "Ridge",
    rules: { description: "Cross for 2 CP foot / 4 CP motorized. Anti-armor and close assault across a ridge each shift two columns in the defender's favor. Does not block ZOC." },
  },
  Slope: {
    title: "Slope",
    rules: { description: "Uphill: 2 CP foot / 4 CP motorized; anti-armor −1, close assault −2 (defender). Downhill: 1 CP foot / 2 CP motorized; anti-armor −1 (defender) but close assault +1 (attacker)." },
  },
  Escarpment: {
    title: "Escarpment",
    rules: { description: "Blocks ZOC. Uphill: 6 CP foot, vehicles prohibited; anti-armor may not fire across it; close assault −3 (defender). Downhill: 4 CP foot / 8 CP motorized; anti-armor −2 (defender), close assault +1 (attacker)." },
  },
  MajorRiver: {
    title: "Major River",
    rules: { description: "Cross for 8 CP foot; vehicles prohibited. Blocks ZOC. A Close Assault across a major river shifts six columns in the defender's favor." },
  },
  MinorRiver: {
    title: "Minor River",
    rules: { description: "Cross for 3 CP foot / 6 CP motorized. A Close Assault across a minor river shifts two columns in the defender's favor." },
  },
};
