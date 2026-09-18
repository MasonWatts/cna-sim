import type { InfoPanelData } from "./lore";

// Overview shown on the "Terrain Key" heading itself.
export const TERRAIN_KEY_INFO: InfoPanelData = {
  title: "The Map (§8)",
  rules: {
    description:
      "The board is a hex grid — each hex is roughly 8 km across, and one Game-Turn is one week of real time. Every hex has a terrain type (the swatches below) that sets its movement cost, vehicle-breakdown risk, stacking limit, and combat-column effects; the shared edges between hexes carry their own features (rivers, ridges, escarpments, slopes, wadis) and routes (roads, tracks, railroads) that modify crossing. Hover any swatch for its specific effects.",
  },
};

// Hybrid info (lore + rules) for Terrain-Key entries, keyed by the legend's terrain / route / edge
// keys. Rules text is grounded in the Terrain Effects Chart (data/reference/terrain.ron, §8.37) and
// §8.4x / §15.3x. Combat "shifts" move columns in the named player's favor. Lore is optional per
// entry (only Salt Marsh has it so far); this is the natural place to add more.
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
    lore: {
      description:
        "Salt marshes — the North African sebkha — are shallow depressions floored with fine, salt-crusted sand. A firm-looking surface often hides a quicksand-like bog that could swallow a vehicle to its axles. Their impassability shaped the campaign: the vast sabkha of the Qattara Depression anchored the southern flank at El Alamein, denying any sweep around the line and forcing a head-on fight.",
      links: [{ label: "Sabkha (Wikipedia)", url: "https://en.wikipedia.org/wiki/Sabkha" }],
    },
    rules: {
      description:
        "§8.44: treacherous salt sand. Except Light Trucks, Recce-type units, and motorcycle infantry, vehicles may enter or leave only along a Road or Track; a prohibited vehicle that enters off-track is Abandoned (§5.33), and no motorized unit or AFV may assault a defender here. Entry costs 3 CP foot / 2 CP motorized. A unit caught defending in a Salt Marsh is bogged — Close Assault shifts one column in the ATTACKER's favor (§15.32); barrage and anti-armor are unaffected.",
    },
  },
  HeavyVegetation: {
    title: "Heavy Vegetation",
    lore: {
      description:
        "The map's handful of vegetated hexes sit along the Cyrenaican coast from Benghazi through Tolmeta (ancient Ptolemais) to Cyrene — the Mediterranean maquis of the Jebel Akhdar, the 'Green Mountain'. Watered by winter rains off the sea, its slopes carry Aleppo pine, juniper, wild olive, cypress, lentisk and myrtle: the lushest corner of Libya and a startling break from the surrounding desert, its cover disrupting the open-country tank warfare typical elsewhere.",
      links: [{ label: "Jebel Akhdar, Libya (Wikipedia)", url: "https://en.wikipedia.org/wiki/Jebel_Akhdar,_Libya" }],
    },
    rules: { description: "Dense growth. Move in for 3 CP. Cover aids the defender: anti-armor and close assault each shift one column in the defender's favor; barrage unaffected. Stacking 6." },
  },
  Rough: {
    title: "Rough",
    rules: { description: "Broken ground. Move in for 3 CP foot / 4 CP motorized. Defender-favorable: barrage −1, anti-armor −1, close assault −2 columns. Stacking 6; elevated breakdown risk." },
  },
  Mountain: {
    title: "Mountain",
    lore: {
      description:
        "The theater's real high ground is the Jebel Akhdar of Cyrenaica — a limestone massif rising some 800 m behind the coast between Benghazi and Derna, laced with steep wadis and few passes. It is the only true mountain country in the coastal fighting zone (the Atlas ranges lie far to the west in Tunisia and Algeria), and its heights and defiles repeatedly channeled the advance and gave defenders commanding ground.",
      links: [{ label: "Jebel Akhdar, Libya (Wikipedia)", url: "https://en.wikipedia.org/wiki/Jebel_Akhdar,_Libya" }],
    },
    rules: { description: "Move in for 4 CP foot / 6 CP motorized. Strongly defensive: barrage & anti-armor −2, close assault −3 columns in the defender's favor. Stacking only 3; very high vehicle breakdown." },
  },
  Delta: {
    title: "Delta",
    lore: {
      description:
        "The cultivated Nile Delta — a dense patchwork of irrigation canals, dykes and fields fanning out north of Cairo to the sea. The green heart of Egypt and the prize behind the Alamein line, its close, water-cut ground favored the defender and broke up armored movement, unlike the open desert to the west.",
      links: [{ label: "Nile Delta (Wikipedia)", url: "https://en.wikipedia.org/wiki/Nile_Delta" }],
    },
    rules: { description: "Cultivated Nile delta. Move in for 2 CP foot / 4 CP motorized. No combat shifts; stacking 6. The lowest breakdown risk on the map." },
  },
  Desert: {
    title: "Desert",
    lore: {
      description:
        "The deep desert south of the narrow coastal strip — the trackless, near-waterless sand-sea and gravel plain that gave the campaign its name. Impassable-looking to conventional armies, it became the highway of raiders and flanking marches: the Long Range Desert Group and Rommel's turning movements navigated it by sun-compass and dead reckoning, and the Great Sand Sea walled off the far southern flank.",
      links: [{ label: "Libyan Desert (Wikipedia)", url: "https://en.wikipedia.org/wiki/Libyan_Desert" }],
    },
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
    lore: {
      description:
        "A wadi is a dry riverbed or ravine, cut by rare desert downpours and usually bone-dry. In the desert war their sunken beds were prized cover — concealing assembly areas, gun lines, and headquarters from view — while the steep banks broke up vehicle movement; a sudden cloudburst could also send a flash flood roaring down one without warning.",
      links: [{ label: "Wadi (Wikipedia)", url: "https://en.wikipedia.org/wiki/Wadi" }],
    },
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
