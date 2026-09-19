import type { InfoPanelData } from "./lore";

// Rules info for the naval panels (§55/§56).
export const PORT_SYSTEM_INFO: InfoPanelData = {
  title: "Ports (§55)",
  rules: {
    description:
      "Each port has an Efficiency Level, shown current/max (e.g. 3/3), and a supply-tonnage capacity per Operations Stage, shown current/base (e.g. 500/500t). The base tonnage is the throughput at full efficiency; the current tonnage scales down with the Efficiency Level (base × efficiency ÷ max), so 1/3 efficiency yields a third of the base tonnage and 0/N is inoperable. Efficiency is knocked down by bombing, air-laid harbour mines (§55.27), or a scuttled wreck blocking the harbour (§55.25) — a held-down port shows ⚠ and is restored or cleared by engineers over time. The ⇅ figure is the port's personnel capacity in Stacking Points that may enter / leave per Operations Stage (§55.13) — it governs how many troops the Commonwealth can transfer by sea through the port; a value of 0 (a minor port) can ship men out but not receive them.",
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

export const MALTA_INFO: InfoPanelData = {
  title: "Malta (§44)",
  rules: {
    description:
      "The British island base off Sicily — no land combat (the Axis may not invade it, §44.3). Its air facilities (four airfields, the Kalafrana flying-boat basin, and the Valetta flying-boat alighting area) host RAF planes that interdict Axis convoys and are the target of Axis air raids (§44.2); each capacity level holds up to 18 planes (§44.14, shown ✈based/capacity) and no SGSUs are used. Flying-boats may only base at the basin or alighting area, and land planes only at airfields (§36.3). Valetta is also a naval base for Commonwealth ships (§30.18). AA/Flak is diverted from the CW pool — one Light (+1) or Heavy (+4) replacement point per month, capped at 48 island-wide (§44.18). Points are held per-field (shown AA<total>, with the Heavy portion in parens) and may be freely redistributed between fields (§44.15); a raid draws only the raided field's flak (§44.24). Escorting fighters can suppress Light flak (one point per three, §40.74) but never Heavy.",
  },
};
