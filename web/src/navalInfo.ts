import type { InfoPanelData } from "./lore";

// Rules info for the naval panels (§55/§56).
export const PORT_SYSTEM_INFO: InfoPanelData = {
  title: "Ports (§55)",
  rules: {
    description:
      "Each port has an Efficiency Level, shown current/max (e.g. 3/3), and a supply-tonnage capacity per Operations Stage, shown current/base (e.g. 500/500t). The base tonnage is the throughput at full efficiency; the current tonnage scales down with the Efficiency Level (base × efficiency ÷ max), so 1/3 efficiency yields a third of the base tonnage and 0/N is inoperable. Efficiency is knocked down by bombing, air-laid harbour mines (§55.27), or a scuttled wreck blocking the harbour (§55.25) — a held-down port shows ⚠ and is restored or cleared by engineers over time.",
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
