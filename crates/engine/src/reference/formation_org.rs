//! §19.3 Formation Organization Chart (§19.31 Allied p135, §19.32 Italian p174, §19.33 German p175).
//!
//! Per formation type, the maximum assigned composition by unit type. The chart draws each slot at
//! its **echelon** (§9.26/§9.27: a division is capped by its brigade/regiment count, a brigade/
//! regiment by its battalion count), so a slot is `(family, echelon, max)`. Transcribed into
//! `docs/formation-org-chart-transcription.md`; this module encodes it for enforcement in
//! `ReassignUnit`. Time-phased rows (the Commonwealth Type I–IV windows, §19.24/§19.25) carry a
//! Game-Turn range. Some slots are "or" alternatives (share a non-zero `alt` group id: the group's
//! combined count may not exceed the largest `max` in the group).

use crate::state::unit::OrgSize;
use serde::{Deserialize, Serialize};

/// The unit-type families the chart distinguishes, at the granularity the engine can classify.
/// (German tank-destroyer + anti-tank fold into `AntiTank`; heavy + light AA into `Aa`.) Also the
/// per-unit type tag carried in the OB (`ObUnit::unit_type`) and shown as the counter glyph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum OrgFamily {
    Tank,
    Infantry,
    /// Motorized infantry (infantry ✕ with wheels). Counts as `Infantry` for §19.3 slots.
    MotInf,
    MgInf,
    Bersaglieri,
    ParaInf,
    Cavalry,
    Artillery,
    AntiTank,
    ArmoredCar,
    Recce,
    Aa,
    Engineer,
    /// Commonwealth "Unit": any non-tank-battalion counter of stacking value 1 or 0.
    AnyUnit,
    /// A headquarters counter with no combat arm (corps HQs, dynamic battle-group pools).
    Hq,
    /// A Tank-Delivery Squadron / Mobile Tank Repair Squad (§22.6).
    Tds,
}

/// A counter-symbol primitive — either a **base branch** (the interior symbol) or an overlaid
/// **modifier**. A unit's `unit_type` is a *set* of these (see `docs/unit-glyphs.md`); the viewer
/// composes the glyph from them, and §19.3 keys off the derived `OrgFamily` (see `primary_arm`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Primitive {
    // Base branches
    Infantry,
    Armor,
    Artillery,
    /// Coastal-defense gun (lens/eye + centre dot). Renders distinctly; still artillery for §19.3.
    CoastalGun,
    AntiTank,
    TankDestroyer,
    AntiAir,
    Cavalry,
    ArmoredCar,
    Recon,
    MachineGun,
    Engineer,
    /// Generic "any 1/0-SP counter" (empty box).
    AnyUnit,
    /// Headquarters (no combat arm).
    Hq,
    /// Tank-Delivery Squadron / Mobile Tank Repair Squad (§22.6).
    Tds,
    /// Railroad-construction unit — restricted to railroads (§23.13). Glyph "RR".
    RailConstruction,
    /// Road-construction unit — restricted to road work (§23.13). Glyph "R".
    RoadConstruction,
    // Modifiers (overlaid on a branch)
    Motorized,
    Motorcycle,
    Airborne,
    SelfPropelled,
    Heavy,
    Bersaglieri,
    Camel,
    Marine,
    Commando,
}

/// The most-specific ("leaf") type of a unit — one variant per physical counter type. This is what
/// the OB/scenario data tags a unit with. `arm()` rolls it up to the §19.3 `OrgFamily`; `glyph()`
/// decomposes it into the render primitives the viewer composes (see `docs/unit-glyphs.md`). `arm()`
/// is defined independently of `glyph()`: e.g. `BersaglieriMotorcycleInfantry` has arm `Bersaglieri`
/// but draws as `[Infantry, Motorcycle]` (no Bersaglieri side-wheels).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum UnitType {
    // Infantry family
    Infantry,
    MotorizedInfantry,
    MachineGunInfantry,
    MotorizedMachineGunInfantry,
    MotorcycleInfantry,
    HeavyWeaponsInfantry,
    MotorizedHeavyWeaponsInfantry,
    MarineInfantry,
    MotorMarineInfantry,
    Commando,
    ParaInfantry,
    HeavyWeaponsParaInfantry,
    BersaglieriInfantry,
    BersaglieriMotorcycleInfantry,
    /// A Bersaglieri regiment HQ in a motorized/armored division: fills the §19.3 Bersaglieri slot
    /// (arm `Bersaglieri`) but the counter is drawn as a support group (armor stadium + ✕).
    BersaglieriSupportGroup,
    /// An armored Bersaglieri battalion: fills the §19.3 Bersaglieri slot (arm `Bersaglieri`) but the
    /// counter is drawn as an armored car.
    BersaglieriArmoredCar,
    // Combined-arms formation
    SupportGroup,
    // Cavalry / recon / armored car
    Cavalry,
    CamelCavalry,
    ArmoredRecon,
    MotorcycleRecon,
    ArmoredCar,
    // Armor / anti-tank
    Armor,
    AntiTank,
    TankDestroyer,
    // Artillery
    Artillery,
    ParaArtillery,
    SelfPropelledArtillery,
    /// Coastal-defense artillery (emplaced/sited); arm `Artillery`, drawn with the coastal-gun glyph.
    CoastalArtillery,
    // Anti-air
    AntiAir,
    HeavyAntiAir,
    // Engineers / construction
    Engineer,
    RailwayConstruction,
    RoadConstruction,
    // Non-combat roles
    Hq,
    TankDelivery,
    AnyUnit,
}

impl UnitType {
    /// The §19.3 arm family this leaf rolls up to.
    pub fn arm(self) -> OrgFamily {
        use OrgFamily as F;
        use UnitType as U;
        match self {
            U::Infantry | U::MarineInfantry | U::Commando | U::SupportGroup => F::Infantry,
            U::MotorizedInfantry | U::MotorMarineInfantry | U::MotorcycleInfantry => F::MotInf,
            U::MachineGunInfantry | U::MotorizedMachineGunInfantry => F::MgInf,
            U::HeavyWeaponsInfantry | U::MotorizedHeavyWeaponsInfantry => F::MgInf,
            U::ParaInfantry | U::HeavyWeaponsParaInfantry => F::ParaInf,
            U::BersaglieriInfantry | U::BersaglieriMotorcycleInfantry | U::BersaglieriSupportGroup | U::BersaglieriArmoredCar => F::Bersaglieri,
            U::Cavalry | U::CamelCavalry => F::Cavalry,
            U::ArmoredRecon | U::MotorcycleRecon => F::Recce,
            U::ArmoredCar => F::ArmoredCar,
            U::Armor => F::Tank,
            U::AntiTank | U::TankDestroyer => F::AntiTank,
            U::Artillery | U::ParaArtillery | U::SelfPropelledArtillery | U::CoastalArtillery => F::Artillery,
            U::AntiAir | U::HeavyAntiAir => F::Aa,
            U::Engineer | U::RailwayConstruction | U::RoadConstruction => F::Engineer,
            U::Hq => F::Hq,
            U::TankDelivery => F::Tds,
            U::AnyUnit => F::AnyUnit,
        }
    }

    /// The render primitives the viewer composes into this leaf's counter glyph.
    pub fn glyph(self) -> Vec<Primitive> {
        use Primitive as P;
        use UnitType as U;
        match self {
            U::Infantry => vec![P::Infantry],
            U::MotorizedInfantry => vec![P::Infantry, P::Motorized],
            U::MachineGunInfantry => vec![P::Infantry, P::MachineGun],
            U::MotorizedMachineGunInfantry => vec![P::Infantry, P::MachineGun, P::Motorized],
            U::MotorcycleInfantry => vec![P::Infantry, P::Motorcycle],
            U::HeavyWeaponsInfantry => vec![P::Infantry, P::Heavy],
            U::MotorizedHeavyWeaponsInfantry => vec![P::Infantry, P::Heavy, P::Motorized],
            U::MarineInfantry => vec![P::Infantry, P::Marine],
            U::MotorMarineInfantry => vec![P::Infantry, P::Marine, P::Motorized],
            U::Commando => vec![P::Infantry, P::Commando],
            U::ParaInfantry => vec![P::Infantry, P::Airborne],
            U::HeavyWeaponsParaInfantry => vec![P::Infantry, P::Airborne, P::Heavy],
            U::BersaglieriInfantry => vec![P::Infantry, P::Bersaglieri],
            U::BersaglieriMotorcycleInfantry => vec![P::Infantry, P::Motorcycle],
            U::BersaglieriSupportGroup => vec![P::Infantry, P::Armor],
            U::BersaglieriArmoredCar => vec![P::ArmoredCar],
            U::SupportGroup => vec![P::Infantry, P::Armor],
            U::Cavalry => vec![P::Cavalry],
            U::CamelCavalry => vec![P::Cavalry, P::Camel],
            U::ArmoredRecon => vec![P::Recon],
            U::MotorcycleRecon => vec![P::Recon, P::Motorcycle],
            U::ArmoredCar => vec![P::ArmoredCar],
            U::Armor => vec![P::Armor],
            U::AntiTank => vec![P::AntiTank],
            U::TankDestroyer => vec![P::TankDestroyer],
            U::Artillery => vec![P::Artillery],
            U::ParaArtillery => vec![P::Artillery, P::Airborne],
            U::SelfPropelledArtillery => vec![P::Armor, P::Artillery],
            U::CoastalArtillery => vec![P::CoastalGun],
            U::AntiAir => vec![P::AntiAir],
            U::HeavyAntiAir => vec![P::AntiAir, P::Heavy],
            U::Engineer => vec![P::Engineer],
            U::RailwayConstruction => vec![P::RailConstruction],
            U::RoadConstruction => vec![P::RoadConstruction],
            U::Hq => vec![P::Hq],
            U::TankDelivery => vec![P::Tds],
            U::AnyUnit => vec![P::AnyUnit],
        }
    }
}

/// Derive the §19.3 arm family (`OrgFamily`) from a unit's primitive set. Composite families
/// (`MgInf`, `Bersaglieri`, `ParaInf`, `MotInf`) are reconstructed from the branch + modifiers so
/// organization rules behave exactly as before. `None` if the set has no arm.
pub fn primary_arm(prims: &[Primitive]) -> Option<OrgFamily> {
    use Primitive as P;
    let has = |p: P| prims.contains(&p);
    if has(P::Hq) {
        return Some(OrgFamily::Hq);
    }
    if has(P::Tds) {
        return Some(OrgFamily::Tds);
    }
    if has(P::AnyUnit) {
        return Some(OrgFamily::AnyUnit);
    }
    if has(P::MachineGun) {
        return Some(OrgFamily::MgInf);
    }
    if has(P::Bersaglieri) {
        return Some(OrgFamily::Bersaglieri);
    }
    if has(P::Infantry) && has(P::Airborne) {
        return Some(OrgFamily::ParaInf);
    }
    if has(P::Infantry) && has(P::Motorized) {
        return Some(OrgFamily::MotInf);
    }
    if has(P::Infantry) {
        return Some(OrgFamily::Infantry);
    }
    // Self-propelled artillery (armor + artillery) counts as artillery for §19.3.
    if has(P::Artillery) || has(P::CoastalGun) {
        return Some(OrgFamily::Artillery);
    }
    if has(P::Armor) {
        return Some(OrgFamily::Tank);
    }
    if has(P::TankDestroyer) || has(P::AntiTank) {
        return Some(OrgFamily::AntiTank);
    }
    if has(P::AntiAir) {
        return Some(OrgFamily::Aa);
    }
    if has(P::Cavalry) {
        return Some(OrgFamily::Cavalry);
    }
    if has(P::ArmoredCar) {
        return Some(OrgFamily::ArmoredCar);
    }
    if has(P::Recon) {
        return Some(OrgFamily::Recce);
    }
    if has(P::Engineer) || has(P::RailConstruction) || has(P::RoadConstruction) {
        return Some(OrgFamily::Engineer);
    }
    None
}

/// The canonical primitive set for a §19.3 arm family — the inverse of `primary_arm`, used to give
/// an untagged unit a glyph from its classified family (fallback only; tagged units use their own set).
pub fn org_family_to_primitives(f: OrgFamily) -> Vec<Primitive> {
    use OrgFamily as F;
    use Primitive as P;
    match f {
        F::Tank => vec![P::Armor],
        F::Infantry => vec![P::Infantry],
        F::MotInf => vec![P::Infantry, P::Motorized],
        F::MgInf => vec![P::Infantry, P::MachineGun],
        F::Bersaglieri => vec![P::Infantry, P::Bersaglieri],
        F::ParaInf => vec![P::Infantry, P::Airborne],
        F::Cavalry => vec![P::Cavalry],
        F::Artillery => vec![P::Artillery],
        F::AntiTank => vec![P::AntiTank],
        F::ArmoredCar => vec![P::ArmoredCar],
        F::Recce => vec![P::Recon],
        F::Aa => vec![P::AntiAir],
        F::Engineer => vec![P::Engineer],
        F::AnyUnit => vec![P::AnyUnit],
        F::Hq => vec![P::Hq],
        F::Tds => vec![P::Tds],
    }
}

impl OrgFamily {
    /// Whether this family is an actual combat arm (subject to §19.3 assignment slots). `Hq`, `Tds`
    /// and the generic `AnyUnit` are not.
    pub fn is_combat_arm(self) -> bool {
        !matches!(self, OrgFamily::Hq | OrgFamily::Tds | OrgFamily::AnyUnit)
    }

    /// The family used for §19.3 slot matching: motorized infantry counts as plain `Infantry`
    /// (the chart doesn't distinguish the motorization modifier).
    pub fn slot_family(self) -> OrgFamily {
        match self {
            OrgFamily::MotInf => OrgFamily::Infantry,
            other => other,
        }
    }
}

/// Coarse echelon at which a slot counts sub-units. Our `OrgSize` cannot tell a `III` regiment from
/// an `X` brigade (both `StandardBrigade`), so both map to `Bde`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Ech {
    Coy,
    Bn,
    /// Regiment or brigade (a division's directly-assigned sub-formation).
    Bde,
}

impl Ech {
    pub fn of(size: OrgSize) -> Ech {
        match size {
            OrgSize::Company => Ech::Coy,
            OrgSize::Battalion => Ech::Bn,
            OrgSize::BattleGroup | OrgSize::StandardBrigade | OrgSize::SuperBrigade | OrgSize::Division => Ech::Bde,
        }
    }
}

/// One composition slot: at most `max` sub-units of `family` at `ech`. Slots sharing a non-zero
/// `alt` form an "or" group.
#[derive(Clone, Copy, Debug)]
pub struct OrgSlot {
    pub family: OrgFamily,
    pub ech: Ech,
    pub max: u32,
    pub alt: u8,
}

const fn slot(family: OrgFamily, ech: Ech, max: u32) -> OrgSlot {
    OrgSlot { family, ech, max, alt: 0 }
}

/// A chart row: the maxima for one formation type, optionally restricted to a Game-Turn window.
#[derive(Clone, Debug)]
pub struct FormationOrgRow {
    /// Substrings matched against the formation HQ's name to select this row (any match). Multiple
    /// HQs share a row when their composition is identical (e.g. the standard Italian infantry
    /// divisions).
    pub hq_names: &'static [&'static str],
    /// Inclusive `(min, max)` Game-Turn window, or `None` for all game.
    pub gt: Option<(u32, u32)>,
    pub slots: &'static [OrgSlot],
}

use Ech::{Bde, Bn, Coy};
use OrgFamily::*;

/// Classify a German unit into a chart `OrgFamily` from its name (works for Organic HQs and
/// battalions alike; German designations are systematic). Order matters: `Panzerjaeger` (anti-tank)
/// and `Panzergrenadier` (infantry) must be tested before the bare `Panzer` (tank).
pub fn german_family(name: &str) -> Option<OrgFamily> {
    let n = name;
    if n.contains("Panzerjaeger") || n.contains("Pak") {
        Some(AntiTank)
    } else if n.contains("Artillery") || n.contains("Arty") {
        // Before the broad infantry match so "1st Afrika Arty Regt" is artillery, not infantry.
        Some(Artillery)
    } else if n.contains("Aufklarungs") || n.contains("Recce") || n.contains("Reconnaissance") {
        Some(Recce)
    } else if n.contains("Flak") {
        Some(Aa)
    } else if n.contains("Pioneer") || n.contains("Engineer") {
        Some(Engineer)
    } else if n.contains("Panzergrenadier")
        || n.contains("Schutzen")
        || n.contains("Machinegewehr")
        || n.contains("Grenadier")
        || n.contains("Gebirgs")
        || n.contains("Inf")
        || n.contains("Oasis")
        || n.contains("Krad")
        || n.contains("Afrika Regt")
        || n.contains("Brandenburg")
    {
        Some(Infantry)
    } else if n.contains("Panzer") {
        Some(Tank)
    } else if n.contains("Fallschirm") || n.contains("Bn (") {
        Some(ParaInf) // Ramcke's "I Bn (Kroh)" etc.
    } else {
        None
    }
}

/// Classify an Italian unit into a chart `OrgFamily` from its name. Order matters: specialised
/// arms (anti-tank, AA, artillery, engineer, bersaglieri, MG, cavalry, armored-car, recce) are
/// tested before the broad infantry / tank catch-alls.
pub fn italian_family(name: &str) -> Option<OrgFamily> {
    let n = name;
    if n.contains("Anti-tank") || n.contains("Antitank") || n.contains("Anti-Tank") {
        Some(AntiTank)
    } else if n.contains("Anti-air") || n.contains("Anti-Air") || n.contains("Anti-aircraft") || n.contains(" AA") {
        Some(Aa)
    } else if n.contains("Artiglieri") || n.contains("Artigliera") || n.contains("Articelere") || n.contains("Artillery") || n.contains("Arty") {
        Some(Artillery)
    } else if n.contains("Engineer") {
        Some(Engineer)
    } else if n.contains("Bersaglieri") {
        Some(Bersaglieri)
    } else if n.contains("Machinegun")
        || n.contains("Mitrg")
        || n.contains("Mitragl")
        || n.contains("MG ")
        || n.ends_with("MG Bn")
        || n.contains("Heavy Wpns")
        || n.contains("Heavy Weapons")
    {
        Some(MgInf)
    } else if n.contains("Armored Car") || n.contains("Autoblindo") || n.contains("RECo") {
        Some(ArmoredCar)
    } else if n.contains("Reconnaissance") || n.contains("Recce") || n.contains("Desert Patrol") {
        Some(Recce)
    } else if n.contains("Camel")
        || n.contains("Sahariano")
        || n.contains("Cavalry")
        || n.contains("Cavalleria")
        || n.contains("Meharisti")
        || n.contains("Squadrone")
        || n.contains("Lanceria")
        || n.contains("Lancer")
        || n.contains("Novara")
        || n.contains("Monferrato")
    {
        Some(Cavalry)
    } else if n.contains("Tank") || n.contains("Corazzat") || n.contains("(L)") || n.contains("(M)") || n.contains("Aresca") || n.contains("Trivioli") {
        Some(Tank)
    } else if n.contains("Fanteri")
        || n.contains("Infantry")
        || n.contains("Libyan")
        || n.contains("Airborne")
        || n.contains("Fascisti")
        || n.contains("CCNN")
        || n.contains("Legion")
        || n.contains("Marine")
        || n.contains("Grenadier")
        || n.contains("Oasis")
        || n.contains("Garrison")
        || n.contains("Coast")
        || n.contains("Regt Mtr")
        || n.contains("PAI")
        || n.contains("Gaf") // Guardia alla Frontiera — frontier infantry
        || n.contains("Inf") // "…Inf. Regt", "…Infantry…"
    {
        Some(Infantry)
    } else {
        None
    }
}

/// Classify a Commonwealth unit into a chart `OrgFamily`. CW infantry battalions are named after
/// their parent regiment (no type keyword), so battalions are classified from their §4.46
/// characteristics `unit_type` plus a tank flag (from weapon kind); brigade/group HQs (characteristic
/// "Headquarters") are classified by name. `unit_type` is the characteristics string ("" if none),
/// `is_tank` whether any weapon group is a Tank system.
pub fn commonwealth_family(unit_type: &str, name: &str, is_tank: bool) -> Option<OrgFamily> {
    // Division sub-formations: armored vs infantry brigades / support groups (counted at Bde echelon).
    if name.contains("Armored Bde") {
        return Some(Tank);
    }
    if name.contains("Bde HQ")
        || name.contains("Brigade HQ")
        || name.contains("Bde Group")
        || name.contains("Support Group")
        || name.contains("Motor Bde")
    {
        return Some(Infantry);
    }
    if is_tank {
        return Some(Tank);
    }
    if name.contains("MG Bn") || name.contains("Machinegun") {
        return Some(MgInf);
    }
    if name.contains("Engineer") || name.contains("Pioneer") || unit_type.contains("Construction") {
        return Some(Engineer);
    }
    if unit_type.contains("Anti-tank") {
        Some(AntiTank)
    } else if unit_type.contains("Anti-air") {
        Some(Aa)
    } else if unit_type.contains("Artillery") {
        Some(Artillery)
    } else if unit_type.contains("Reconnaissance") {
        Some(Recce)
    } else if unit_type.contains("Infantry") || name.contains("Guards") {
        Some(Infantry)
    } else {
        None
    }
}

/// §19.33 German rows. Divisions cap brigade/regiment sub-formations (`Bde`) and any loose
/// battalions (`Bn`); regiments/brigades cap battalions.
pub static GERMAN_ROWS: &[FormationOrgRow] = &[
    // 5 Leichte (pre-transform, before the 21st Panzer HQ arrives GT 58): loose battalions.
    FormationOrgRow {
        hq_names: &["5th Leichte Panzer Div HQ"],
        gt: Some((1, 57)),
        slots: &[
            slot(Tank, Bn, 2),
            slot(Infantry, Bn, 2),
            slot(AntiTank, Bn, 2),
            slot(Artillery, Bn, 1),
            slot(Aa, Bn, 2),
            slot(Recce, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // 21st Panzer Division (post-transform): one panzer regiment + one infantry regiment.
    FormationOrgRow {
        hq_names: &["21st Panzer Div HQ"],
        gt: None,
        slots: &[
            slot(Tank, Bde, 1),
            slot(Infantry, Bde, 1),
            slot(Infantry, Bn, 2),
            slot(MgInf, Bn, 2), // 2nd/8th Machinegewehr (motorized MG) bns
            slot(AntiTank, Bn, 2),
            slot(Artillery, Bn, 2),
            slot(Aa, Bn, 2),
            slot(Recce, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // 15th Panzer Division: panzer regiment + the 15th Schützen Brigade (+ divisional Schützen regt).
    FormationOrgRow {
        hq_names: &["15th Panzer Div HQ"],
        gt: None,
        slots: &[
            slot(Tank, Bde, 1),
            // §19.33 "X:3": the 15th Schützen Brigade grouping = up to 3 infantry regiments
            // (historically the 200th + 104th + 115th Schützen).
            slot(Infantry, Bde, 3),
            slot(AntiTank, Bn, 1),
            slot(Artillery, Bn, 3),
            slot(Recce, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // 90th Leichte Afrika Division: 2 infantry regiments + loose battalions.
    FormationOrgRow {
        hq_names: &["90th Leichte Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Infantry, Bn, 2),
            slot(AntiTank, Bn, 1),
            slot(Artillery, Bn, 1),
            slot(Recce, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // 164th Leichte Afrika Division: 3 panzergrenadier regiments + an artillery regiment.
    FormationOrgRow {
        hq_names: &["164th Leichte Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 3),
            slot(AntiTank, Bn, 1),
            slot(Artillery, Bde, 1),
            slot(Aa, Bn, 1),
            slot(Recce, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // Ramcke parachute brigade.
    FormationOrgRow { hq_names: &["Ramcke Bde HQ"], gt: None, slots: &[slot(ParaInf, Bn, 5)] },
    // Sonderverband 288.
    FormationOrgRow {
        hq_names: &["Sonderverband 288 HQ"],
        gt: None,
        // 2/288 Gebirgs (mot inf) + 3/288 Schwere (mot heavy-wpn) + 4/288 Machinegewehr (mot MG).
        slots: &[slot(Infantry, Bn, 1), slot(MgInf, Bn, 2), slot(AntiTank, Bn, 1), slot(Engineer, Coy, 1)],
    },
];

/// §19.32 Italian rows. Maxima are the full historical divisional organization (Italian divisions
/// are fixed-org): 2 infantry regiments + divisional artillery/MG/AT/engineer for the standard
/// (semi-motorized) infantry divisions; the armored, motorized, Libyan, parachute and special
/// formations carry their own compositions. Verified against the OB by
/// `formation_org_italian_ob_within_maxima`.
pub static ITALIAN_ROWS: &[FormationOrgRow] = &[
    // Standard (semi-motorized) infantry divisions.
    FormationOrgRow {
        hq_names: &[
            "64th Catanzaro Div HQ",
            "62nd Marmarica Div HQ",
            "17th Pavia Div HQ",
            "25th Bologna Div HQ",
            "55th Savona Div HQ",
            "27 Brescia Div HQ",
        ],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Artillery, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(AntiTank, Coy, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // 63 Cirene / 61 Sirte: standard template + a Bersaglieri motorcycle recce company (§4.46 counter).
    FormationOrgRow {
        hq_names: &["63rd Cirene Div HQ", "61st Sirte Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Artillery, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(AntiTank, Coy, 1),
            slot(Engineer, Bn, 1),
            slot(Recce, Coy, 1),
        ],
    },
    // Sabratha: standard + a Bersaglieri motorcycle recce company (its "60 Brs" counter).
    FormationOrgRow {
        hq_names: &["60th Sabratha Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Artillery, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(AntiTank, Coy, 1),
            slot(Recce, Coy, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // Blackshirt (CCNN) divisions: 1st & 2nd carry a Bersaglieri company; 4th does not.
    FormationOrgRow {
        hq_names: &["1st CCNN Div HQ", "2nd CCNN Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Artillery, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(Recce, Coy, 1), // the 201st/202nd Bersaglieri motorcycle recce company
            slot(Engineer, Bn, 1),
        ],
    },
    FormationOrgRow {
        hq_names: &["4th CCNN Div HQ"],
        gt: None,
        slots: &[slot(Infantry, Bde, 2), slot(Artillery, Bn, 1), slot(MgInf, Bn, 1), slot(Engineer, Bn, 1)],
    },
    // Pistoia: infantry + Bersaglieri company + the III Monferrato armored-car squadron.
    FormationOrgRow {
        hq_names: &["16th Pistoia Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Artillery, Bn, 1),
            slot(Bersaglieri, Coy, 1),
            slot(ArmoredCar, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // Armored divisions.
    FormationOrgRow {
        hq_names: &["Ariete (132nd) Div HQE"],
        gt: None,
        slots: &[
            slot(Tank, Bde, 1),
            slot(Tank, Bn, 2),
            slot(Bersaglieri, Bde, 1),
            slot(ArmoredCar, Bn, 1),
            slot(AntiTank, Bn, 1),
            slot(Artillery, Bn, 1),
        ],
    },
    FormationOrgRow {
        hq_names: &["Littorio (133rd) Div HQ"],
        gt: None,
        slots: &[
            slot(Tank, Bde, 1),
            slot(Bersaglieri, Bde, 1),
            slot(ArmoredCar, Bn, 1),
            slot(Artillery, Bn, 3),
        ],
    },
    // Motorized infantry divisions.
    FormationOrgRow {
        hq_names: &["101st Trieste Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Bersaglieri, Bde, 1),
            slot(Bersaglieri, Bn, 1),
            slot(Aa, Bn, 1),
            slot(Artillery, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    FormationOrgRow {
        hq_names: &["102nd Trento Div HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 2),
            slot(Bersaglieri, Bde, 1),
            slot(Aa, Bn, 1),
            slot(AntiTank, Bn, 1),
            slot(Artillery, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(Engineer, Bn, 1),
        ],
    },
    // Folgore (parachute) division.
    FormationOrgRow {
        hq_names: &["185th Folgore Div HQ"],
        gt: None,
        slots: &[slot(ParaInf, Bde, 2), slot(Artillery, Bn, 1)],
    },
    // Maletti group (Libyan): may assign a third infantry regiment (§19.32).
    FormationOrgRow {
        hq_names: &["Maletti Div HQ"],
        gt: None,
        slots: &[slot(Infantry, Bde, 3), slot(Cavalry, Bde, 1), slot(Artillery, Bn, 2)],
    },
    // Libyan infantry divisions.
    FormationOrgRow {
        hq_names: &["1st Libyan Infantry HQ", "2nd Libyan Infantry HQ"],
        gt: None,
        slots: &[slot(Infantry, Bde, 2), slot(Artillery, Bn, 1), slot(AntiTank, Coy, 1)],
    },
    // Giovani Fascisti division.
    FormationOrgRow {
        hq_names: &["136th Giovani Fascisti Div HQ"],
        gt: None,
        slots: &[slot(Infantry, Bde, 1), slot(Artillery, Bn, 1)],
    },
    // Libyan Tank Command = the §19.32 "Tank Group" (Game-Turn 1–26): two tank regiments.
    FormationOrgRow {
        hq_names: &["Libyan Tank Command HQ"],
        gt: Some((1, 26)),
        slots: &[slot(Tank, Bde, 2)],
    },
    // RECAM (Reggruppamento Esplorante — not "armored" for armored-division assignment).
    FormationOrgRow {
        hq_names: &["RECAM HQ"],
        gt: None,
        slots: &[slot(ArmoredCar, Bn, 1), slot(Artillery, Bn, 1), slot(MgInf, Coy, 1), slot(Engineer, Bn, 1)],
    },
    // Saharan Detachment.
    FormationOrgRow {
        hq_names: &["Saharan Detachment HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bn, 2),
            slot(Cavalry, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(MgInf, Coy, 1),
            slot(Artillery, Coy, 1),
        ],
    },
];

/// The Commonwealth armored / Army-Tank brigade HQs (share the time-phased §19.31 armored-brigade
/// rows).
static ARMORED_BDE_HQS: [&str; 8] = [
    "4th Armored Bde HQ",
    "7th Armored Bde HQ",
    "8th Armored Bde HQ",
    "9th Armored Bde HQ",
    "22nd Armored Bde HQ",
    "23rd Armored Bde HQ",
    "24th Armored Bde HQ",
    "1st Army Tank Bde HQ",
];

/// §19.31 Commonwealth rows. Divisions cap their brigade sub-formations (`Bde`: armored brigades →
/// `Tank`, infantry brigades / support groups → `Infantry`) and divisional support battalions (`Bn`);
/// brigades cap their battalions. Maxima are the full historical organization (the union across the
/// §19.31 Type I–IV time windows); the Type windows themselves (§19.24/§19.25, deferral item **D4**)
/// are not yet gated per Game-Turn. Verified by `formation_org_commonwealth_ob_within_maxima`.
pub static COMMONWEALTH_ROWS: &[FormationOrgRow] = &[
    // Infantry divisions (3 infantry brigades + divisional support).
    FormationOrgRow {
        hq_names: &[
            "4th Indian Div HQ",
            "44th Infantry Div HQ",
            "50th Infantry Div HQ",
            "51st Infantry Div HQ",
            "70th Infantry Div HQ",
            "10th Indian Div HQ",
            "2nd South African Div HQ",
            "6th Australian Div HQ",
        ],
        gt: None,
        slots: &[
            slot(Infantry, Bde, 3),
            slot(Artillery, Bn, 3),
            slot(Recce, Bn, 1),
            slot(MgInf, Bn, 1),
            slot(Aa, Bn, 1),
            slot(AntiTank, Bn, 1),
            slot(Infantry, Bn, 1),
        ],
    },
    // Armored divisions (2 armored brigades + 1 infantry brigade / support group + a recce bn).
    // Time-phased into two windows so the single Infantry/Bde support-group slot can swap: the 7th
    // Armored Division's 7th Support Group HQ (present to 2/68) is replaced by the 7th Motor Bde HQ
    // (arrives 2/68) per §4.44B p117 note c. Windows cover all game-turns, so the 8th/10th Armored
    // Divisions (whose support groups do not swap) are unaffected.
    FormationOrgRow {
        hq_names: &["7th Armored Div HQE", "8th Armored Div HQ", "10th Armored Div HQ"],
        gt: Some((1, 67)),
        slots: &[slot(Tank, Bde, 2), slot(Infantry, Bde, 1), slot(Recce, Bn, 1)],
    },
    FormationOrgRow {
        hq_names: &["7th Armored Div HQE", "8th Armored Div HQ", "10th Armored Div HQ"],
        gt: Some((68, u32::MAX)),
        slots: &[slot(Tank, Bde, 2), slot(Infantry, Bde, 1), slot(Recce, Bn, 1)],
    },
    // Armored / Army-Tank brigades — §19.31 time-phased (D4): Type I (GT 1–18) = 3 tank battalions;
    // Type II (GT 19–70 and 92+) = + 1 infantry battalion; Type III (GT 71–91) = + 1 artillery
    // battalion. (Type II spans two windows, so it is listed as two rows with identical slots.)
    FormationOrgRow { hq_names: &ARMORED_BDE_HQS, gt: Some((1, 18)), slots: &[slot(Tank, Bn, 3)] },
    FormationOrgRow {
        hq_names: &ARMORED_BDE_HQS,
        gt: Some((19, 70)),
        slots: &[slot(Tank, Bn, 3), slot(Infantry, Bn, 1)],
    },
    FormationOrgRow {
        hq_names: &ARMORED_BDE_HQS,
        gt: Some((71, 91)),
        slots: &[slot(Tank, Bn, 3), slot(Infantry, Bn, 1), slot(Artillery, Bn, 1)],
    },
    FormationOrgRow {
        hq_names: &ARMORED_BDE_HQS,
        gt: Some((92, u32::MAX)),
        slots: &[slot(Tank, Bn, 3), slot(Infantry, Bn, 1)],
    },
    // Support Group / Motor Brigade Group.
    FormationOrgRow {
        hq_names: &["7th Support Group HQ"],
        gt: None,
        slots: &[slot(Infantry, Bn, 2), slot(Artillery, Bn, 1), slot(AntiTank, Bn, 1), slot(Recce, Bn, 1)],
    },
    // Allied battle-group brigades (individual compositions).
    FormationOrgRow {
        hq_names: &["1st Free French Bde HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bn, 5),
            slot(Aa, Bn, 1),
            slot(AntiTank, Coy, 1),
            slot(Artillery, Bn, 1),
            slot(ArmoredCar, Bn, 1), // the 1st Free French Flying Column (armored car)
        ],
    },
    FormationOrgRow {
        hq_names: &["2nd Free French Bde HQ"],
        gt: None,
        slots: &[slot(Infantry, Bn, 2), slot(AntiTank, Coy, 1)],
    },
    FormationOrgRow {
        hq_names: &["1st Greek Bde HQ"],
        gt: None,
        slots: &[slot(Infantry, Bn, 3), slot(Artillery, Bn, 1)],
    },
    FormationOrgRow {
        hq_names: &["Polish Brigade HQ"],
        gt: None,
        slots: &[
            slot(Infantry, Bn, 3),
            slot(MgInf, Coy, 1), // Kopanski MG Coy

            slot(Artillery, Bn, 1),
            slot(AntiTank, Coy, 1),
            slot(Recce, Bn, 1),
        ],
    },
];
