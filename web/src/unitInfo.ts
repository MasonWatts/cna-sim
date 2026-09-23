import type { InfoPanelData } from "./lore";

// Overview shown on the "Unit Key" heading itself: what a counter is and how to read its symbol.
export const UNIT_KEY_INFO: InfoPanelData = {
  title: "Unit Counters (§4)",
  rules: {
    description:
      "Every land unit is a cardboard counter (§4.4/§4.44) printed with a branch symbol (the glyphs below), its TOE Strength Points, Basic Morale, and organisation size. The symbol tells you what the unit is — infantry, armour, artillery, and so on — and drives which combat it fights: close assault (§15), barrage (§13), or anti-armor (§14). Hover any swatch for what that type means and typical examples.",
  },
  lore: {
    description:
      "The glyphs follow the standard military-mapping convention — an 'X' box for infantry, an oval for armour, a dot for artillery — the same NATO-style symbology armies use so any staff officer can read an unfamiliar map at a glance. CNA renders its own hand-drawn take on that scheme.",
    links: [
      { label: "NATO Joint Military Symbology", url: "https://en.wikipedia.org/wiki/NATO_Joint_Military_Symbology" },
      { label: "Military organization", url: "https://en.wikipedia.org/wiki/Military_organization" },
    ],
  },
};

// Per-type info for the Unit-Key swatches, keyed by the primitive set joined with "+" (matching the
// keys in UnitKey's CURATED table). Each entry: a plain-language explanation of the branch plus
// representative example units from the North African order of battle. Types without a curated entry
// fall back to a label-only panel in the component.
export const UNIT_TYPE_INFO: Record<string, InfoPanelData> = {
  Infantry: {
    title: "Infantry",
    rules: {
      description:
        "Foot infantry — the line troops that hold ground and make close assaults (§15). The campaign's most numerous counter. Examples: British and Indian rifle battalions, German and Italian infantry battalions.",
    },
  },
  Artillery: {
    title: "Artillery",
    rules: {
      description:
        "Tube artillery that fires barrage attacks at range to soften a hex before an assault (§13). Examples: British Field Regiments (25-pdr) and Medium Regiments, German and Italian artillery battalions.",
    },
  },
  CoastalGun: {
    title: "Coastal defense",
    rules: {
      description:
        "Fixed coastal-defense batteries that fire on warships attacking their port (§30/§55) and add to the fortress in a land fight. Examples: the Tobruk and Bardia coastal gun positions.",
    },
  },
  Armor: {
    title: "Armor",
    rules: {
      description:
        "Tank units — the desert war's main striking arm, strong in anti-armor fire (§14) and mobile close assault. Examples: British RTR cruiser and infantry-tank battalions, German Panzer battalions (Pz III/IV), Italian M13/40 battalions.",
    },
  },
  AntiAir: {
    title: "Anti-air",
    rules: {
      description:
        "Anti-aircraft guns that fire at aircraft (flak, §40/§44) and, for heavier pieces, double as anti-tank/ground weapons. Examples: British Light AA (Bofors 40mm), the German 88mm Flak.",
    },
  },
  AntiTank: {
    title: "Anti-tank",
    rules: {
      description:
        "Towed anti-tank guns whose job is stopping armour (§14); weak in other roles. Examples: British 2-pdr (often portee-mounted), German and Italian anti-tank companies.",
    },
  },
  "Infantry+MachineGun": {
    title: "Machine-gun",
    rules: {
      description:
        "Machine-gun infantry — concentrated automatic weapons providing heavy defensive fire. Examples: British/Commonwealth MG battalions (e.g. the Northumberland Fusiliers).",
    },
  },
  "Infantry+Heavy+Motorized": {
    title: "Motor heavy-weapons",
    rules: {
      description: "Truck-borne heavy-weapons infantry — mortars and heavy machine guns carried by lorry for pace with mobile columns.",
    },
  },
  "Infantry+Armor": {
    title: "Support group",
    rules: {
      description:
        "A combined-arms 'support group' — infantry, artillery and armour packaged into one mobile task force. Example: the 7th Armoured Division Support Group.",
    },
  },
  Engineer: {
    title: "Engineer",
    rules: {
      description:
        "Combat engineers who build and breach fortifications and minefields and handle construction tasks (§24/§26). Examples: Royal Engineers field companies, German Pioniere.",
    },
  },
  "Infantry+Bersaglieri": {
    title: "Bersaglieri",
    rules: {
      description:
        "Bersaglieri — elite Italian light infantry, historically fast-marching and often truck- or motorcycle-mounted in the armoured divisions. Examples: the Bersaglieri regiments of Ariete and Trieste.",
    },
  },
  Recon: {
    title: "Armored recon",
    rules: {
      description:
        "Armoured reconnaissance — fast, lightly-armoured units that scout ahead and screen flanks. Examples: German Aufklärungs (recon) detachments, divisional recce squadrons.",
    },
  },
  Hq: {
    title: "Headquarters",
    rules: {
      description:
        "A headquarters counter — it commands its formation and lends its Command Point Allowance to subordinate units (§19), and persists as the formation's cadre even at 0 TOE (§19.62). Examples: division and brigade HQs.",
    },
  },
  ArmoredCar: {
    title: "Armored car",
    rules: {
      description:
        "Wheeled armoured cars for reconnaissance and screening across open desert. Examples: the British 11th Hussars and King's Dragoon Guards.",
    },
  },
  "Infantry+Airborne": {
    title: "Parachute infantry",
    rules: {
      description: "Airborne (parachute) infantry — elite light troops. Examples: German Fallschirmjäger, British/Commonwealth parachute units.",
    },
  },
  "Infantry+Airborne+Heavy": {
    title: "Heavy para infantry",
    rules: { description: "The heavy-weapons element of parachute infantry — support weapons attached to an airborne battalion." },
  },
  Cavalry: {
    title: "Cavalry",
    rules: { description: "Horse cavalry — mounted troops used for mobility and screening in the earlier/peripheral theatres. Examples: Indian cavalry regiments." },
  },
  "Cavalry+Camel": {
    title: "Camel cavalry",
    rules: { description: "Camel-mounted cavalry, suited to long-range desert movement where horses could not be watered. Examples: Commonwealth camel corps units." },
  },
  "Infantry+Marine": {
    title: "Marine infantry",
    rules: { description: "Naval/marine infantry raised for coastal and amphibious work. Example: the Italian San Marco marine regiment." },
  },
  Tds: {
    title: "Tank delivery",
    rules: {
      description:
        "A Tank Delivery Section — not a combat unit but a marker that ferries replacement tanks forward to rebuild armoured battalions. It has no attack of its own.",
    },
  },
  "Recon+Motorcycle": {
    title: "Motorcycle recon",
    rules: { description: "Motorcycle-mounted reconnaissance — very fast on roads and tracks. Example: German Kradschützen (motorcycle) battalions." },
  },
  RailConstruction: {
    title: "RR construction",
    rules: { description: "Railway construction troops that build and repair the rail line feeding supply forward. Examples: Royal Engineers railway construction companies." },
  },
  "Infantry+Marine+Motorized": {
    title: "Motor marine",
    rules: { description: "Motorised marine infantry — naval infantry carried by lorry for mobile operations." },
  },
  "Infantry+Commando": {
    title: "Commando",
    rules: {
      description:
        "Commandos — raiding light infantry for coastal and behind-the-lines operations (§27). Example: the British Layforce commandos.",
    },
  },
  TankDestroyer: {
    title: "Tank destroyer",
    rules: { description: "Self-propelled tank destroyers — anti-tank guns on a tracked, mobile mount, specialised for killing armour." },
  },
  RoadConstruction: {
    title: "Road construction",
    rules: { description: "Construction/engineer units that build and upgrade tracks and roads to speed movement and supply." },
  },
  "Infantry+Motorized": {
    title: "Motorized infantry",
    rules: {
      description:
        "Lorried (truck-mounted) infantry with far greater movement than foot troops (§6.17), able to keep pace with armour. Examples: German motorized infantry, British motor battalions.",
    },
  },
  AnyUnit: {
    title: "Generic unit",
    rules: { description: "A generic placeholder symbol used where any unit type may apply (e.g. in reference charts), not a specific branch." },
  },
  SelfPropelled: {
    title: "Self-propelled",
    rules: { description: "Self-propelled guns — artillery or anti-tank weapons mounted on a tracked chassis for mobility with the armoured spearhead." },
  },
};
