// Turn the recorder's raw command/outcome Debug strings (e.g.
//   action  "RefuelUnit { unit: UnitId(187) }"
//   outcome "UnitRefueled(6)")
// into friendly, human-readable prose for the Current Action panel. Purely presentational and
// viewer-side: it parses the Debug text and resolves unit ids to names from the current frame, so it
// needs no recorder change or regen. Commands without a bespoke template fall back to a generic
// humanised rendering (spaced name + cleaned parameters), so every command still reads sensibly.

export type ActionText = { headline: string; detail?: string };

// ---- small parsers over the Rust Debug string --------------------------------------------------
const cmdName = (s: string): string => (s.match(/^(\w+)/)?.[1] ?? s);
const uid = (s: string, key?: string): number | null => {
  const re = key ? new RegExp(`${key}:\\s*UnitId\\((\\d+)\\)`) : /UnitId\((\d+)\)/;
  const m = s.match(re);
  return m ? Number(m[1]) : null;
};
const hex = (s: string, key?: string): string | null => {
  const re = key ? new RegExp(`${key}:\\s*HexCoord \\{ col: (\\d+), row: (\\d+) \\}`) : /HexCoord \{ col: (\d+), row: (\d+) \}/;
  const m = s.match(re);
  return m ? `(${m[1]},${m[2]})` : null;
};
const str = (s: string, key: string): string | null => s.match(new RegExp(`${key}:\\s*"([^"]+)"`))?.[1] ?? null;
const int = (s: string, key: string): number | null => {
  const m = s.match(new RegExp(`${key}:\\s*(\\d+)`));
  return m ? Number(m[1]) : null;
};
const word = (s: string, key: string): string | null => s.match(new RegExp(`${key}:\\s*([A-Za-z]\\w*)`))?.[1] ?? null;
// The (first) integer inside an outcome like "UnitRefueled(6)" / "Repaired(0)".
const outNum = (s: string): number | null => {
  const m = s.match(/\((-?\d+)\)/);
  return m ? Number(m[1]) : null;
};
// Spaced Title Case from a PascalCase command name: "RefuelUnit" -> "Refuel Unit".
const spaced = (name: string) => name.replace(/([a-z0-9])([A-Z])/g, "$1 $2");

/** Friendly description of one recorded action. `un` resolves a unit id to its display name; `sh`
 * resolves a fleet ship_index to a ship name. */
export function describeAction(
  action: string,
  outcome: string,
  un: (id: number) => string,
  sh: (index: number) => string = (i) => `ship #${i}`,
): ActionText {
  if (!action || action === "start") return { headline: "Scenario start", detail: "Initial deployment of forces." };
  const name = cmdName(action);
  const u = () => { const i = uid(action); return i == null ? "a unit" : un(i); };
  const ship = () => { const i = int(action, "ship_index"); return i == null ? "a warship" : sh(i); };
  const on = outNum(outcome);

  switch (name) {
    case "MoveStep": {
      const mode = word(action, "mode");
      return { headline: `Moved ${u()} to ${hex(action, "to")}`, detail: mode ? `${mode === "Motorized" ? "Motorized" : "Foot"} movement.` : undefined };
    }
    case "FormationMove":
      return { headline: `Moved formation ${un(uid(action, "formation") ?? -1)} to ${hex(action, "to")}`, detail: "The whole formation advanced together." };
    case "MoveOffMap": {
      const dir = word(action, "direction");
      const arr = outcome.match(/Arrived\("([^"]+)"\)/)?.[1];
      return {
        headline: `Marched ${u()} ${dir === "TowardMap" ? "toward the map" : "off-map"}`,
        detail: arr ? `Arrived in the ${arr} box.` : "Advanced one stage along the off-map track.",
      };
    }
    case "RefuelUnit":
      return { headline: `Refueled ${u()}`, detail: on != null ? `Drew fuel from supply — fuel now ${on}.` : "Drew fuel from supply." };
    case "RearmUnit":
      return { headline: `Rearmed ${u()}`, detail: on != null ? `Drew ammunition from supply — ammo now ${on}.` : "Drew ammunition from supply." };
    case "Repair": {
      const kind = word(action, "kind");
      return { headline: `Repaired ${u()}`, detail: `Repaired ${on ?? 0} broken ${kind ? kind.toLowerCase() : "item(s)"}.` };
    }
    case "RepairShip":
      return { headline: `Repaired ${ship()}`, detail: on != null ? `Battle damage patched — gun rating now ${on}.` : "Battle damage patched." };
    case "ConstructSupplyDump": {
      const dummy = /dummy: true/.test(action);
      return { headline: `Built a ${dummy ? "dummy (decoy)" : "supply"} dump at ${hex(action, "hex")}`, detail: `${u()} unloaded supplies to form the dump.` };
    }
    case "ArmSquadron":
      return { headline: `Armed a squadron`, detail: `Loaded ${int(action, "count") ?? "its"} planes with ordnance for a strike.` };
    case "ReadySquadron":
      return { headline: `Readied a squadron`, detail: `Brought ${int(action, "count") ?? "its"} planes to readiness.` };
    case "RefitSquadron":
      return { headline: `Refit a squadron`, detail: on != null ? `Replaced losses — ${on} planes after refit.` : "Replaced plane losses." };
    case "AssignSquadronPlanes": {
      const t = str(action, "aircraft_type"), cls = word(action, "class"), n = int(action, "count");
      return { headline: `Assigned aircraft to a squadron`, detail: `${n ?? ""} × ${t ?? "planes"}${cls ? ` (${cls})` : ""} allotted.` };
    }
    case "DeploySgsu": {
      const nat = word(action, "nationality"), cls = word(action, "class");
      return { headline: `Deployed an air squadron at ${hex(action, "at")}`, detail: `${nat ?? ""} ${cls ?? "squadron"} brought into play.` };
    }
    case "BaseSquadronOffMap":
      return { headline: `Based a squadron off-map`, detail: `Stationed at ${str(action, "facility") ?? "an off-map field"}.` };
    case "AssignPilot":
      return { headline: `Assigned a pilot`, detail: `A rating-${int(action, "rating") ?? "?"} pilot took a squadron.` };
    case "AssignShip": {
      const h = hex(action);
      return { headline: `Positioned ${ship()}`, detail: h ? `Sailed to ${h}.` : "Set its station." };
    }
    case "PoisonWell":
      return { headline: `Poisoned the well at ${hex(action, "hex")}`, detail: `${/true/.test(outcome) ? "The well is now poisoned" : "The attempt failed (needs a die roll of 1)"}.` };
    case "SweetenWell":
      return { headline: `Sweetened the well at ${hex(action, "hex")}`, detail: `${/true/.test(outcome) ? "The well is drinkable again" : "The attempt failed"}.` };
    case "TrainUnit":
      return { headline: `Set ${u()} to train`, detail: "Green troops train up toward their target morale." };
    case "AttachUnit":
      return { headline: `Attached ${u()}`, detail: `Now rides with formation ${un(uid(action, "parent") ?? -1)}.` };
    case "DetachUnit":
      return { headline: `Detached ${u()}`, detail: "Split off to operate independently." };
    case "ReassignUnit":
      return { headline: `Reassigned ${u()}`, detail: `Now belongs to formation ${un(uid(action, "parent") ?? -1)}.` };
    case "CloseAssault":
      return { headline: `Close assault into ${hex(action, "defender_hex") ?? hex(action)}`, detail: "A ground attack to take the hex." };
    case "Barrage":
      return { headline: `Artillery barrage on ${hex(action)}`, detail: "Ranged fire to soften the target." };
    case "AntiArmor":
      return { headline: `Anti-armor fire at ${hex(action)}`, detail: "Guns engaging enemy tanks." };
    case "Patrol":
      return { headline: `Flew a Combat Air Patrol`, detail: "Fighters standing guard over the hex." };
  }

  // ---- generic fallback: spaced name + cleaned parameters --------------------------------------
  const cleaned = action
    .replace(/UnitId\((\d+)\)/g, (_m, n) => un(Number(n)))
    .replace(/SquadronId\((\d+)\)/g, (_m, n) => `squadron ${n}`)
    .replace(/HexCoord \{ col: (\d+), row: (\d+) \}/g, "($1,$2)")
    .replace(/^\w+\s*\{?\s*/, "")
    .replace(/\s*\}$/, "")
    .trim();
  const outClean = outcome
    .replace(/HexCoord \{ col: (\d+), row: (\d+) \}/g, "($1,$2)")
    .replace(/SquadronId\((\d+)\)/g, "squadron $1");
  return { headline: spaced(name), detail: cleaned ? `${cleaned}${outClean ? ` → ${outClean}` : ""}` : outClean || undefined };
}
