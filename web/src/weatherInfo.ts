import type { InfoPanelData } from "./lore";

// Rules info for the Weather panel (§29). One entry explains the system + Region bands; the others
// explain each condition's effects. Grounded in §29.1/§29.3/§29.4/§29.5.

export const WEATHER_SYSTEM_INFO: InfoPanelData = {
  title: "Weather (§29)",
  rules: {
    description:
      "Each Game-Turn the weather is rolled per map Region (§29.1), and a Region's condition applies to every hex in it. The five Regions are vertical column bands — A (cols 1–33), B (34–66), C (67–99), D (100–132), E (133–165). Conditions are Normal, Hot, Sandstorm, and Rainstorm; each alters movement, vehicle breakdown, water/fuel supply, and air operations.",
  },
};

export const WEATHER_CONDITION_INFO: Record<string, InfoPanelData> = {
  Normal: {
    title: "Normal weather",
    rules: { description: "Baseline conditions — standard movement, breakdown, supply, and air operations apply." },
  },
  Hot: {
    title: "Hot weather (§29.3)",
    rules: {
      description:
        "Water requirements for all units are doubled (§29.35). Fuel and Water levels (except in wells and oases) lose 5% to evaporation each Weather Determination Phase (§29.34). Breakdown checks shift one column higher (§29.33). Construction sites need water rations (§29.32).",
    },
  },
  Sandstorm: {
    title: "Sandstorm (§29.4)",
    rules: {
      description:
        "No aircraft may fly into or out of a sandstorm hex (§29.43). All movement costs are doubled (§29.44); a unit spending 50%+ of its CPA moving in the storm shifts its breakdown column one higher (§29.45). No construction (§29.42). Sandstorms stop at the coast (§29.46) and can damage grounded aircraft (§29.47).",
    },
  },
  Rainstorm: {
    title: "Rainstorm (§29.5)",
    rules: {
      description:
        "No aircraft may fly into or out of a rainstorm hex (§29.52). Roads degrade to Track movement/breakdown (§29.56); wadi hexsides become uncrossable except by road (§29.55); rivers without a road or rail crossing are impassable (§29.57); vehicles and motorized units may not enter (or must stay put in) delta hexes off-road (§29.58). No construction (§29.54) — but all depleted wells refill (§29.53).",
    },
  },
};
