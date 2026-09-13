//! Combat resolvers. First: Anti-Armor Fire (§14). Rolls flow through the seeded RNG;
//! the `*_with_dice` variants take the roll explicitly for deterministic testing.
//!
//! Implemented for anti-armor: §14.3 terrain column shifts, the §14.6 phasing-player −1-row
//! modifier, §14.47 self-propelled-gun absorption (AP + Vulnerability), and §14.46/§14.5
//! destroyed-tank markers (knocked-out tanks become retrievable, enemy-capturable equipment).
//! §14.21 withholding is honored: defending units passed as `withheld` take no Anti-Armor damage.
//! §14.12 defensive/mutual fire is supported at the command layer — the `AntiArmor` command is not
//! gated to the phasing player, so the non-phasing side fires it at the assaulting armor's hex, and
//! two calls (one per side) give a mutual exchange. §14.48 halftrack absorption is modeled in
//! `apply_armor_damage`. Deferred (later increment): the §15.16 per-TOE-point close-assault
//! commitment split.

use serde::{Deserialize, Serialize};
use crate::reference::crt::{
    BarrageResult, BarrageTargetClass, CloseAssaultResult, CloseAssaultSide, MoraleModifier,
};
use crate::reference::terrain::CombatMod;
use crate::reference::units::WeaponKind;
use crate::state::game::GameState;
use crate::state::map::{HexCoord, HexsideKind};
use crate::state::unit::{Composition, GunPosition, OrgSize, RatingKind, UnitId};

/// A close-assault result with no effect (used before/without a CRT result).
const NO_RESULT: CloseAssaultResult = CloseAssaultResult {
    loss_percent: 0,
    captured: false,
    engaged: false,
    retreat_hexes: 0,
    in_contact: true,
};

/// The outcome of one anti-armor attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntiArmorOutcome {
    pub dice: u8,
    pub raw_points: i32,
    pub actual_points: i32,
    /// §14.3 terrain column shift in the defender's favour (≤ 0): in-hex terrain effect plus any
    /// crossed-hexside effect. Applied to `actual_points` to get the CRT column.
    pub terrain_shift: i32,
    /// The Anti-Armor CRT column actually used: `(actual_points + terrain_shift).max(0)` (§14.35).
    pub column: i32,
    pub damage: i32,
    /// Armor TOE Strength Points removed from the target.
    pub armor_toe_removed: u32,
    /// §14.6 note: whether the Phasing-Player −1-row modifier was applied to the dice read.
    pub phasing_penalty: bool,
}

impl GameState {
    /// Resolve an anti-armor attack by `attackers` against `target_hex`, rolling the dice.
    pub fn resolve_anti_armor(
        &mut self,
        attackers: &[UnitId],
        target_hex: HexCoord,
    ) -> AntiArmorOutcome {
        let dice = self.rng.two_dice_raw();
        self.resolve_anti_armor_with_dice(attackers, target_hex, dice)
    }

    /// Resolve an anti-armor attack with an explicit dice read (11..=66).
    pub fn resolve_anti_armor_with_dice(
        &mut self,
        attackers: &[UnitId],
        target_hex: HexCoord,
        dice: u8,
    ) -> AntiArmorOutcome {
        self.resolve_anti_armor_core(attackers, target_hex, dice, &[])
    }

    /// Resolve an anti-armor attack with an explicit dice read and a set of defending units the
    /// defender has **withheld** from combat (§14.21) — those units take no Anti-Armor damage.
    pub(crate) fn resolve_anti_armor_core(
        &mut self,
        attackers: &[UnitId],
        target_hex: HexCoord,
        dice: u8,
        withheld: &[UnitId],
    ) -> AntiArmorOutcome {
        // Total Raw Anti-Armor Points across attackers (§11.34), then Actual (÷10). §14.34
        // excludes any attacker whose fire crosses a prohibited hexside.
        let raw: i32 = attackers
            .iter()
            .filter_map(|id| self.units.get(id))
            .filter(|u| {
                u.hex()
                    .is_none_or(|fh| !self.anti_armor_fire_blocked(fh, target_hex))
            })
            .map(|u| u.raw_strength(&self.reference, RatingKind::AntiArmor))
            .sum();
        let actual = (raw + 5) / 10; // round half up; raw ≤ 4 → 0 (uses the 0* column)

        // §14.32/14.33: terrain shifts the Anti-Armor column in the defender's favour; §14.35: a
        // shift below row 1 uses the 0 row.
        let terrain_shift = self.anti_armor_terrain_shift(attackers, target_hex);
        let column = (actual + terrain_shift).max(0);

        // §14.6 note: the Phasing Player fires one row lower (rows 11,12 unaffected). Anti-Armor
        // fire in Movement & Combat is the phasing player's fire (§14.12); infer it from the
        // firing units' side against the active (phasing) player.
        let phasing = attackers
            .iter()
            .filter_map(|id| self.units.get(id))
            .any(|u| u.nationality.side() == self.active_player);
        let effective_dice = if phasing {
            crate::reference::crt::AntiArmorCrt::phasing_row_dice(dice)
        } else {
            dice
        };
        let phasing_penalty = phasing && effective_dice != dice;

        let damage = self
            .reference
            .anti_armor_crt
            .as_ref()
            .map(|crt| crt.damage(effective_dice, column, raw))
            .unwrap_or(0);

        let armor_toe_removed = self.apply_armor_damage(target_hex, damage, withheld);
        AntiArmorOutcome {
            dice,
            raw_points: raw,
            actual_points: actual,
            terrain_shift,
            column,
            damage,
            armor_toe_removed,
            phasing_penalty,
        }
    }

    /// §14.34: whether Anti-Armor fire from `from` into `to` is prohibited by the crossed hexside:
    /// across a Ridge not crossed by a road or track; *down* a Slope not crossed by a road or
    /// track; or *down* an Escarpment not crossed by a track. ("Down" = firing from the hexside's
    /// high side.) A Road/Track (an unfinished road counts as a track, §24.52) negates it.
    pub fn anti_armor_fire_blocked(&self, from: HexCoord, to: HexCoord) -> bool {
        let sides = self.map.hexsides_on(from, to);
        if sides.is_empty() {
            return false;
        }
        let road_or_track = sides.iter().any(|h| {
            matches!(h.kind, HexsideKind::Road | HexsideKind::Track | HexsideKind::UnfinishedRoad)
        });
        let track = sides
            .iter()
            .any(|h| matches!(h.kind, HexsideKind::Track | HexsideKind::UnfinishedRoad));
        sides.iter().any(|h| match h.kind {
            HexsideKind::Ridge => !road_or_track,
            HexsideKind::Slope => h.high_side == Some(from) && !road_or_track, // firing down
            HexsideKind::Escarpment => h.high_side == Some(from) && !track,    // firing down
            _ => false,
        })
    }

    /// §14.3 terrain column shift on Anti-Armor fire against `target_hex` (≤ 0, favouring the
    /// defender): the in-hex terrain effect (§14.32; a hex has one base terrain, so no §14.33
    /// "best of several" choice until fortifications are modeled) PLUS the effect of any hexside
    /// the fire crosses (§14.33 additive) — taking the defender's best across the attacker hexes.
    fn anti_armor_terrain_shift(&self, attackers: &[UnitId], target_hex: HexCoord) -> i32 {
        let terrain = self
            .map
            .terrain(target_hex)
            .and_then(|t| self.reference.terrain.get(&t))
            .map(|p| combat_mod_shift(p.anti_armor))
            .unwrap_or(0);
        // §14.33: hex terrain and fortifications are NOT cumulative — the defender takes the best.
        let in_hex = terrain.min(self.fort_shift(target_hex));
        let mut hexside = 0;
        for a in attackers {
            if let Some(ah) = self.units.get(a).and_then(|u| u.hex()) {
                let s: i32 = self
                    .map
                    .hexsides_on(ah, target_hex)
                    .iter()
                    .filter_map(|hs| self.reference.hexsides.get(&hs.resolve(ah, target_hex)))
                    .map(|p| combat_mod_shift(p.anti_armor))
                    .sum();
                hexside = hexside.min(s);
            }
        }
        in_hex + hexside
    }

    /// Apply Damage Points to the armored TOE in `hex` (§14.42–43). Each armor TOE point
    /// absorbs its Armor-Protection rating in damage; we remove highest-absorption points first
    /// so the fewest TOE are lost (a default choice; a player/AI may override later).
    ///  - §14.47: a self-propelled gun (Artillery/Anti-Tank weapon that also carries an
    ///    Armor-Protection rating) absorbs at its Armor Protection **plus** its Vulnerability.
    ///  - §14.48: truck-transported halftrack infantry (Organic units whose §4.44 characteristics
    ///    carry an Armor-Protection rating — the German "i" Schützen) are anti-armor-affectable and
    ///    absorb at that rating; if the hit side's *only* affectable points in the hex are
    ///    halftracks, they **double** it. A destroyed halftrack TOE point also loses its
    ///    transporting Truck Points.
    fn apply_armor_damage(&mut self, hex: HexCoord, damage: i32, withheld: &[UnitId]) -> u32 {
        if damage <= 0 {
            return 0;
        }
        // Collect every absorbing element: (unit, Some(group)|None-for-halftrack, absorb-per-TOE,
        // is-tank, is-halftrack, nationality, system). §14.21: a unit withheld from both Anti-Armor
        // and Close Assault is not affected by Anti-Armor fire and is excluded here.
        type Absorber = (UnitId, Option<usize>, i32, bool, bool, crate::reference::units::Nationality, String);
        let mut armored: Vec<Absorber> = Vec::new();
        for id in self.units_in_hex(hex) {
            if withheld.contains(&id) {
                continue;
            }
            let Some(unit) = self.units.get(&id) else {
                continue;
            };
            let nat = unit.nationality;
            match &unit.composition {
                Composition::Weapons { groups } => {
                    for (gi, g) in groups.iter().enumerate() {
                        let ws = self.reference.weapons_systems.get(&nat).and_then(|m| m.get(&g.system));
                        let ap = ws.and_then(|w| w.armor_protection).map(|r| r.value).unwrap_or(0);
                        // §14.47 self-propelled gun: a gun weapon that also has armor protection.
                        let sp_bonus = ws
                            .filter(|w| matches!(w.kind, WeaponKind::Artillery | WeaponKind::AntiTank))
                            .and_then(|w| w.vulnerability)
                            .map(|r| r.value)
                            .unwrap_or(0);
                        let is_tank = ws.map(|w| matches!(w.kind, WeaponKind::Tank)).unwrap_or(false);
                        let absorb = ap + sp_bonus;
                        if absorb > 0 && g.count > 0 {
                            armored.push((id, Some(gi), absorb, is_tank, false, nat, g.system.clone()));
                        }
                    }
                }
                Composition::Organic { current_toe } => {
                    // §14.48 halftrack: an Organic infantry unit whose characteristics carry an
                    // Armor-Protection rating, but only while truck-transported (riding its
                    // halftracks). Dismounted (no Truck Points) it is ordinary infantry, unaffected.
                    let ap = self
                        .reference
                        .unit_characteristics
                        .get(&nat)
                        .and_then(|m| m.get(&unit.id_code))
                        .and_then(|c| c.armor_protection)
                        .map(|r| r.value)
                        .unwrap_or(0);
                    let mounted = unit.trucks.light + unit.trucks.medium + unit.trucks.heavy > 0;
                    if ap > 0 && *current_toe > 0 && mounted {
                        armored.push((id, None, ap, false, true, nat, String::new()));
                    }
                }
            }
        }
        // §14.48: double halftrack absorption when the hit side's only affectable points are halftracks.
        if !armored.is_empty() && armored.iter().all(|a| a.4) {
            for a in armored.iter_mut() {
                a.2 *= 2;
            }
        }
        armored.sort_by_key(|a| std::cmp::Reverse(a.2)); // highest absorption first

        let mut remaining = damage;
        let mut removed = 0u32;
        for (id, gi, absorb, is_tank, halftrack, nat, system) in armored {
            if remaining <= 0 {
                break;
            }
            let count = match (&self.units.get(&id).unwrap().composition, gi) {
                (Composition::Weapons { groups }, Some(g)) => groups[g].count,
                (Composition::Organic { current_toe }, None) => *current_toe,
                _ => 0,
            };
            if count == 0 {
                continue;
            }
            let needed = ((remaining + absorb - 1) / absorb) as u32; // ceil(remaining / absorb)
            let take = needed.min(count);
            if let Some(unit) = self.units.get_mut(&id) {
                match (&mut unit.composition, gi) {
                    (Composition::Weapons { groups }, Some(g)) => groups[g].count -= take,
                    (Composition::Organic { current_toe }, None) => {
                        *current_toe -= take;
                        // §14.48: the halftrack's transporting Truck Points are lost with it —
                        // scale the unit's trucks to the surviving TOE fraction.
                        let keep = count - take;
                        unit.trucks.light = unit.trucks.light * keep / count;
                        unit.trucks.medium = unit.trucks.medium * keep / count;
                        unit.trucks.heavy = unit.trucks.heavy * keep / count;
                    }
                    _ => {}
                }
            }
            // §14.46/§14.51: destroyed *tanks* leave retrievable Destroyed Tank Markers (owned by
            // their side until captured, §14.52); other destroyed vehicles are simply removed.
            if is_tank && take > 0 {
                self.record_destroyed_tank(hex, nat, system, take);
            }
            let _ = halftrack;
            removed += take;
            remaining -= (take as i32) * absorb;
        }
        removed
    }

    /// Whether a unit holds any Tank-kind weapon TOE (for the §42.24a recon reveal order).
    pub(crate) fn unit_has_tank(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else { return false };
        let Composition::Weapons { groups } = &u.composition else { return false };
        let table = self.reference.weapons_systems.get(&u.nationality);
        groups.iter().any(|g| {
            table
                .and_then(|m| m.get(&g.system))
                .map(|w| matches!(w.kind, WeaponKind::Tank))
                .unwrap_or(false)
        })
    }

    /// Whether a unit is a "gun" for §15.84 (has Artillery or Anti-Tank weapons). AA/Flak
    /// are excluded (exempt from Vulnerability losses, §15.84b).
    pub(crate) fn is_gun_unit(&self, id: UnitId) -> bool {
        let Some(u) = self.units.get(&id) else {
            return false;
        };
        let Composition::Weapons { groups } = &u.composition else {
            return false;
        };
        let table = self.reference.weapons_systems.get(&u.nationality);
        groups.iter().any(|g| {
            table
                .and_then(|m| m.get(&g.system))
                .map(|w| matches!(w.kind, WeaponKind::Artillery | WeaponKind::AntiTank))
                .unwrap_or(false)
        })
    }

    /// Apply Vulnerability damage to a set of gun units (§15.84b/c): each gun TOE point
    /// absorbs its Vulnerability rating (§14.42 mechanics); highest-Vulnerability points are
    /// removed first. Only Artillery/Anti-Tank groups are affected. Returns TOE removed.
    pub(crate) fn apply_vulnerability_damage(&mut self, units: &[UnitId], damage: i32) -> u32 {
        if damage <= 0 {
            return 0;
        }
        let mut pts: Vec<(UnitId, usize, i32)> = Vec::new();
        for &id in units {
            let Some(unit) = self.units.get(&id) else {
                continue;
            };
            let Composition::Weapons { groups } = &unit.composition else {
                continue;
            };
            let nat = unit.nationality;
            for (gi, g) in groups.iter().enumerate() {
                let ws = self.reference.weapons_systems.get(&nat).and_then(|m| m.get(&g.system));
                let is_gun = ws
                    .map(|w| matches!(w.kind, WeaponKind::Artillery | WeaponKind::AntiTank))
                    .unwrap_or(false);
                let vuln = ws.and_then(|w| w.vulnerability).map(|r| r.value).unwrap_or(0);
                if is_gun && vuln > 0 && g.count > 0 {
                    pts.push((id, gi, vuln));
                }
            }
        }
        pts.sort_by_key(|a| std::cmp::Reverse(a.2)); // highest Vulnerability first
        let mut remaining = damage;
        let mut removed = 0u32;
        for (id, gi, vuln) in pts {
            if remaining <= 0 {
                break;
            }
            let count = match &self.units.get(&id).unwrap().composition {
                Composition::Weapons { groups } => groups[gi].count,
                _ => 0,
            };
            if count == 0 {
                continue;
            }
            let needed = ((remaining + vuln - 1) / vuln) as u32; // ceil(remaining / vuln)
            let take = needed.min(count);
            if let Some(unit) = self.units.get_mut(&id) {
                if let Composition::Weapons { groups } = &mut unit.composition {
                    groups[gi].count -= take;
                }
            }
            removed += take;
            remaining -= (take as i32) * vuln;
        }
        removed
    }
}

/// The outcome of one barrage against a target unit (§12.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarrageOutcome {
    pub dice: u8,
    pub actual_points: i32,
    pub class: BarrageTargetClass,
    pub result: BarrageResult,
    /// §12.33 terrain column shift applied (negative = benefits the defender).
    pub terrain_shift: i32,
    /// TOE Strength Points removed from the target.
    pub toe_removed: u32,
    /// §12.46: first-line Truck Points destroyed by the separate Truck-column roll.
    pub trucks_removed: u32,
    /// Whether the target became Pinned.
    pub pinned: bool,
}

impl GameState {
    /// Barrage Target Class of a unit (§12.6). Organic → Infantry; a weapons unit holding
    /// any tank (armor protection but no barrage rating) → Armor; otherwise Gun. Truck
    /// class is deferred until transport is modeled.
    pub fn barrage_class(&self, target: UnitId) -> BarrageTargetClass {
        let Some(u) = self.units.get(&target) else {
            return BarrageTargetClass::Infantry;
        };
        match &u.composition {
            Composition::Organic { .. } => BarrageTargetClass::Infantry,
            Composition::Weapons { groups } => {
                let table = self.reference.weapons_systems.get(&u.nationality);
                let armored = groups.iter().any(|g| {
                    table
                        .and_then(|m| m.get(&g.system))
                        .map(|ws| matches!(ws.kind, WeaponKind::Tank | WeaponKind::Reconnaissance))
                        .unwrap_or(false)
                });
                if armored {
                    BarrageTargetClass::Armor
                } else {
                    BarrageTargetClass::Gun
                }
            }
        }
    }

    /// Resolve a barrage by `attackers` against `target`, rolling the dice.
    pub fn resolve_barrage(&mut self, attackers: &[UnitId], target: UnitId) -> BarrageOutcome {
        let dice = self.rng.two_dice_raw();
        self.resolve_barrage_with_dice(attackers, target, dice)
    }

    /// Resolve a barrage with an explicit dice read (11..=66).
    pub fn resolve_barrage_with_dice(
        &mut self,
        attackers: &[UnitId],
        target: UnitId,
        dice: u8,
    ) -> BarrageOutcome {
        let raw: i32 = attackers
            .iter()
            .filter_map(|id| self.units.get(id))
            .map(|u| u.raw_strength(&self.reference, RatingKind::Barrage))
            .sum();
        let actual = (raw + 5) / 10;
        self.resolve_barrage_points(actual, target, dice)
    }

    /// Resolve a barrage from an explicit number of *actual* Barrage Strength Points against
    /// `target`, with dice read `dice` (11..=66). Used both by unit barrages (which derive the
    /// points from Raw Strength) and by Commonwealth off-shore naval bombardment, whose Gun Rating
    /// is used directly as Actual Artillery Strength Points (§30.22).
    pub fn resolve_barrage_points(
        &mut self,
        actual: i32,
        target: UnitId,
        dice: u8,
    ) -> BarrageOutcome {
        let class = self.barrage_class(target);
        let terrain_shift = self.barrage_terrain_shift(target);
        let result = self
            .reference
            .barrage_crt
            .as_ref()
            .map(|crt| crt.result_shifted(class, actual, dice, terrain_shift))
            .unwrap_or(BarrageResult::NoEffect);

        // Apply the result (§12.6 footnotes). Guns/Trucks are never Pinned.
        let armor_or_infantry =
            matches!(class, BarrageTargetClass::Infantry | BarrageTargetClass::Armor);
        let (toe_loss, pin) = match result {
            BarrageResult::NoEffect => (0, false),
            BarrageResult::Pinned => (0, true),
            BarrageResult::One => (1, armor_or_infantry),
            BarrageResult::Two => (2, armor_or_infantry),
        };
        let toe_removed = if toe_loss > 0 {
            self.remove_toe(target, toe_loss)
        } else {
            0
        };
        if pin {
            if let Some(u) = self.units.get_mut(&target) {
                u.status.pinned = true;
            }
        }
        // §12.46: a separate roll on the Trucks column determines how many first-line Truck
        // Points attached to the target are destroyed (only if any are present). Guns/Trucks
        // are never Pinned, so only the point loss applies.
        let has_trucks = self
            .units
            .get(&target)
            .map(|u| u.trucks.light + u.trucks.medium + u.trucks.heavy > 0)
            .unwrap_or(false);
        let trucks_removed = if has_trucks {
            let truck_dice = self.rng.two_dice_raw();
            let truck_result = self
                .reference
                .barrage_crt
                .as_ref()
                .map(|crt| crt.result_shifted(BarrageTargetClass::Truck, actual, truck_dice, 0))
                .unwrap_or(BarrageResult::NoEffect);
            let loss = match truck_result {
                BarrageResult::One => 1,
                BarrageResult::Two => 2,
                _ => 0,
            };
            if loss > 0 {
                self.remove_trucks(target, loss)
            } else {
                0
            }
        } else {
            0
        };
        BarrageOutcome {
            dice,
            actual_points: actual,
            class,
            result,
            terrain_shift,
            toe_removed,
            trucks_removed,
            pinned: pin,
        }
    }

    /// Remove up to `n` first-line Truck Points from a unit, distributing losses as evenly as
    /// possible among truck types (§12.46), returning how many were removed.
    fn remove_trucks(&mut self, target: UnitId, n: u32) -> u32 {
        let Some(u) = self.units.get_mut(&target) else {
            return 0;
        };
        let mut removed = 0;
        for _ in 0..n {
            // Pick the currently-largest non-empty type so losses stay even.
            let t = &mut u.trucks;
            let pick = [
                (t.heavy, 2u8),
                (t.medium, 1u8),
                (t.light, 0u8),
            ]
            .into_iter()
            .filter(|(c, _)| *c > 0)
            .max_by_key(|(c, _)| *c);
            match pick {
                Some((_, 2)) => t.heavy -= 1,
                Some((_, 1)) => t.medium -= 1,
                Some((_, 0)) => t.light -= 1,
                _ => break,
            }
            removed += 1;
        }
        removed
    }

    /// §12.33 terrain column shift for a barrage: the target hex's Barrage `CombatMod`
    /// (negative = benefits the defender). §12.34: terrain and fortifications are NOT cumulative —
    /// the defender takes the single best defensive value.
    fn barrage_terrain_shift(&self, target: UnitId) -> i32 {
        let Some(hex) = self.effective_hex(target) else {
            return 0;
        };
        let terrain = self
            .map
            .terrain(hex)
            .and_then(|t| self.reference.terrain.get(&t))
            .map(|p| combat_mod_shift(p.barrage))
            .unwrap_or(0);
        terrain.min(self.fort_shift(hex))
    }

    /// Remove up to `n` TOE Strength Points from a unit, returning how many were removed.
    /// For weapons units the defender chooses which points to lose; the engine's default
    /// removes from groups in order (a player/AI may override later).
    fn remove_toe(&mut self, target: UnitId, n: u32) -> u32 {
        let Some(u) = self.units.get_mut(&target) else {
            return 0;
        };
        match &mut u.composition {
            Composition::Organic { current_toe } => {
                let take = (*current_toe).min(n);
                *current_toe -= take;
                take
            }
            Composition::Weapons { groups } => {
                let mut remaining = n;
                let mut removed = 0;
                for g in groups.iter_mut() {
                    if remaining == 0 {
                        break;
                    }
                    let take = g.count.min(remaining);
                    g.count -= take;
                    remaining -= take;
                    removed += take;
                }
                removed
            }
        }
    }
}

/// Per-side morale for a close assault (§15.6 / §17.2): the representative Basic Morale
/// and Cohesion Level (largest-unit rule), the morale-table roll and modifier, the
/// resulting Adjusted Morale Rating (clamped to [-3, +3], §17.23), and whether the side
/// Surrendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideMorale {
    pub basic: i32,
    pub cohesion: i32,
    pub roll: u8,
    pub modifier: MoraleModifier,
    pub adjusted: i32,
    pub surrendered: bool,
}

/// The outcome of one close assault (§15.79). Reuses the CRT's per-side result for the
/// loss %, capture, engaged, retreat, and in-contact flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseAssaultOutcome {
    /// Final Close Assault Differential (attacker CA-off − defender CA-def, §15.28-aware).
    pub differential: i32,
    pub attacker_strength: i32,
    pub defender_strength: i32,
    /// Combined-arms reduction (§15.4) applied to each side's Actual strength.
    pub attacker_combined_arms: i32,
    pub defender_combined_arms: i32,
    /// §15.54: the defender is a lone unit with no Defensive Close Assault Rating
    /// (+3 columns to the attacker, §15.51 suppressed, attacker takes no casualties).
    pub zero_def_loner: bool,
    /// Base column from the differential, and the column actually used after all shifts.
    /// `morale_shift`/`size_shift`/`terrain_shift` are the component column shifts.
    pub base_column: usize,
    pub morale_shift: i32,
    pub size_shift: i32,
    pub terrain_shift: i32,
    pub adjusted_column: usize,
    pub attacker_morale: SideMorale,
    pub defender_morale: SideMorale,
    /// The two-dice combat reads (11..66) rolled by each side.
    pub attacker_read: u8,
    pub defender_read: u8,
    pub attacker: CloseAssaultResult,
    pub defender: CloseAssaultResult,
    /// §15.77: the assault fell in the Overrun columns (defender losses round up, guns
    /// become vulnerable).
    pub is_overrun: bool,
    pub attacker_toe_removed: u32,
    pub defender_toe_removed: u32,
    /// §15.84b/c: TOE removed from defending guns by Vulnerability losses (part of
    /// `defender_toe_removed`).
    pub defender_vulnerability_removed: u32,
    /// Attacker casualties taken prisoner by the defender (attacker-block Capt, §15.89).
    pub attacker_prisoners: u32,
    /// Defender casualties taken prisoner by the attacker (defender-block Capt, §15.89).
    pub defender_prisoners: u32,
    /// The Prisoners-table d6 rolled for each Capt result (if any).
    pub attacker_prisoner_roll: Option<u8>,
    pub defender_prisoner_roll: Option<u8>,
}

/// The dice sum (2..12) implied by a straight two-dice read (11..66).
fn dice_sum(read: u8) -> u8 {
    read / 10 + read % 10
}

/// Actual Close Assault Strength Points from raw (§15.28): raw < 5 → 0; if `both_under_10`
/// (both sides < 10 raw) use raw directly; otherwise round raw ÷ 10 (half up).
fn ca_actual(raw: i32, both_under_10: bool) -> i32 {
    if raw < 5 {
        0
    } else if both_under_10 {
        raw
    } else {
        (raw + 5) / 10
    }
}
/// Convert a terrain `CombatMod` into a signed close-assault column shift (right = toward
/// the attacker). `Prohibited` is a legality matter (§15.34), not a shift → 0 here.
fn combat_mod_shift(m: CombatMod) -> i32 {
    match m {
        CombatMod::NoShift | CombatMod::Prohibited => 0,
        CombatMod::Shift(n) => n,
    }
}

/// Round `sum / n` to the nearest whole number (§6.27), away from zero on ties.
fn round_div(sum: i32, n: i32) -> i32 {
    if n == 0 {
        return 0;
    }
    (sum as f64 / n as f64).round() as i32
}


/// §15.53 organizational-size column shift, signed positive toward the attacker: compares
/// each side's largest unit and shifts in favor of the larger organization.
fn org_size_shift(attacker: OrgSize, defender: OrgSize) -> i32 {
    let ra = attacker.stacking_points();
    let rd = defender.stacking_points();
    if ra > rd {
        size_53_magnitude(attacker, defender)
    } else if rd > ra {
        -size_53_magnitude(defender, attacker)
    } else {
        0
    }
}

/// §15.53 table magnitude, given `larger` strictly outranks `smaller`.
fn size_53_magnitude(larger: OrgSize, smaller: OrgSize) -> i32 {
    use OrgSize::*;
    match (larger, smaller) {
        (Division, SuperBrigade) => 1,
        (Division, StandardBrigade) | (Division, BattleGroup) => 2,
        (Division, Battalion) => 4,
        (Division, Company) => 8,
        (SuperBrigade, Battalion) | (StandardBrigade, Battalion) | (BattleGroup, Battalion) => 2,
        (SuperBrigade, Company) | (StandardBrigade, Company) | (BattleGroup, Company) => 4,
        (Battalion, Company) => 2,
        _ => 0,
    }
}

impl GameState {
    /// Actual Close Assault strength for a set of units: Σ raw ratings ÷ 10 (§11.34).
    /// `offensive` picks the Close-Assault Offensive vs Defensive rating.
    pub fn close_assault_strength(&self, units: &[UnitId], offensive: bool) -> i32 {
        let kind = if offensive {
            RatingKind::CloseAssaultOff
        } else {
            RatingKind::CloseAssaultDef
        };
        let raw: i32 = units
            .iter()
            .filter_map(|id| self.units.get(id))
            .map(|u| u.raw_strength(&self.reference, kind))
            .sum();
        (raw + 5) / 10
    }

    /// Raw Close Assault Strength Points for a set of units (Σ rating × count).
    pub fn close_assault_raw(&self, units: &[UnitId], offensive: bool) -> i32 {
        let kind = if offensive {
            RatingKind::CloseAssaultOff
        } else {
            RatingKind::CloseAssaultDef
        };
        units
            .iter()
            .filter_map(|id| self.units.get(id))
            .map(|u| u.raw_strength(&self.reference, kind))
            .sum()
    }

    /// Final Close Assault Differential (attacker offensive − defender defensive), applying
    /// §15.28 (raw < 5 → 0 actual; both sides raw < 10 → raw as actual) and the §15.4
    /// combined-arms reduction to each side's actual strength.
    pub fn close_assault_differential(&self, attackers: &[UnitId], defenders: &[UnitId]) -> i32 {
        let a_raw = self.close_assault_raw(attackers, true);
        let d_raw = self.close_assault_raw(defenders, false);
        let both_under_10 = a_raw < 10 && d_raw < 10;
        let a = (ca_actual(a_raw, both_under_10) - self.combined_arms_reduction(attackers)).max(0);
        let d = (ca_actual(d_raw, both_under_10) - self.combined_arms_reduction(defenders)).max(0);
        a - d
    }

    /// Combined-arms reduction (§15.4): tanks (armor, no barrage — excludes SP artillery)
    /// need equal supporting TOE (infantry/MG/heavy weapons — approximated here as all
    /// non-tank TOE) from the *same hex*; unsupported tank TOE reduces the side's Actual
    /// Close-Assault Strength by ceil(unsupported / 3) per hex, capped at 4 for the side.
    pub fn combined_arms_reduction(&self, units: &[UnitId]) -> i32 {
        use std::collections::HashMap;
        // (tank TOE, support TOE) per hex.
        let mut by_hex: HashMap<Option<HexCoord>, (u32, u32)> = HashMap::new();
        for &id in units {
            let Some(u) = self.units.get(&id) else {
                continue;
            };
            let hex = self.effective_hex(id);
            let entry = by_hex.entry(hex).or_insert((0, 0));
            match &u.composition {
                Composition::Organic { current_toe } => entry.1 += *current_toe,
                Composition::Weapons { groups } => {
                    let table = self.reference.weapons_systems.get(&u.nationality);
                    for g in groups {
                        let is_tank = table
                            .and_then(|m| m.get(&g.system))
                            .map(|ws| ws.kind == WeaponKind::Tank)
                            .unwrap_or(false);
                        // Only tanks require support; non-tank weapons (guns/AA/recce)
                        // neither require nor provide combined-arms support (§15.4).
                        if is_tank {
                            entry.0 += g.count;
                        }
                    }
                }
            }
        }
        let mut reduction = 0i32;
        for (_, (tanks, support)) in by_hex {
            let unsupported = tanks.saturating_sub(support);
            if unsupported > 0 {
                reduction += unsupported.div_ceil(3) as i32; // ceil(unsupported / 3)
            }
        }
        reduction.min(4)
    }

    /// Resolve a close assault, rolling the morale checks and combat dice for both sides.
    /// Roll order: attacker morale, defender morale, attacker combat, defender combat.
    pub fn resolve_close_assault(
        &mut self,
        attackers: &[UnitId],
        defenders: &[UnitId],
    ) -> CloseAssaultOutcome {
        let attacker_morale_read = self.rng.two_dice_raw();
        let defender_morale_read = self.rng.two_dice_raw();
        let attacker_read = self.rng.two_dice_raw();
        let defender_read = self.rng.two_dice_raw();
        self.resolve_close_assault_with_rolls(
            attackers,
            defenders,
            attacker_morale_read,
            defender_morale_read,
            attacker_read,
            defender_read,
        )
    }

    /// Resolve a close assault with explicit two-dice reads (11..66) for each of the two
    /// morale checks and the two combat rolls. Applies the morale column shift (§15.6),
    /// Surrender (§17.25/§17.26), losses, prisoners (§15.89), and status effects.
    ///
    /// Simplifications (documented deferrals): the representative Morale/Cohesion per side
    /// uses the largest unit (no division-averaging §15.64c, no Rommel §17.28). Terrain (§15.3),
    /// unit-size (§15.5), and morale (§15.6) column shifts are applied to the base column here.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_close_assault_with_rolls(
        &mut self,
        attackers: &[UnitId],
        defenders: &[UnitId],
        attacker_morale_read: u8,
        defender_morale_read: u8,
        attacker_read: u8,
        defender_read: u8,
    ) -> CloseAssaultOutcome {
        // Raw & actual strengths (§15.28 + §15.4 combined arms) and the base differential.
        let a_raw_off = self.close_assault_raw(attackers, true);
        // §15.21/§15.84a: defending guns do not fight the close-assault melee, so they are
        // excluded from the defender's differential strength. In an Overrun they are added
        // to the loss total (see the loss section below).
        let def_gun_ids: Vec<UnitId> = defenders.iter().copied().filter(|&id| self.is_gun_unit(id)).collect();
        let def_troop_ids: Vec<UnitId> = defenders.iter().copied().filter(|&id| !self.is_gun_unit(id)).collect();
        // §13.28: a defender that Retreated Before Assault into this hex may not add its Close
        // Assault Ratings to the defense — but its TOE is still included when taking % losses
        // (so it stays in `def_troop_ids`/`participating`, only out of the strength calc).
        let def_strength_ids: Vec<UnitId> = def_troop_ids
            .iter()
            .copied()
            .filter(|&id| !self.units.get(&id).map(|u| u.retreated_before_assault).unwrap_or(false))
            .collect();
        let d_raw_troops = self.close_assault_raw(&def_strength_ids, false);
        // §52.51/52: units deprived of water defend at half their raw strength.
        let d_deprived_raw: i32 = def_strength_ids
            .iter()
            .filter(|&&id| self.unit_water_deprived(id))
            .filter_map(|id| self.units.get(id))
            .map(|u| u.raw_strength(&self.reference, RatingKind::CloseAssaultDef))
            .sum();
        let d_raw_troops = d_raw_troops - d_deprived_raw / 2;
        let both_under_10 = a_raw_off < 10 && d_raw_troops < 10;
        let attacker_combined_arms = self.combined_arms_reduction(attackers);
        let defender_combined_arms = self.combined_arms_reduction(defenders);
        let attacker_strength =
            (ca_actual(a_raw_off, both_under_10) - attacker_combined_arms).max(0);
        let defender_strength =
            (ca_actual(d_raw_troops, both_under_10) - defender_combined_arms).max(0);
        let differential = attacker_strength - defender_strength;
        let base_column = crate::reference::crt::CloseAssaultCrt::column_for_differential(differential);

        // §15.54: a lone defending unit with no Defensive Close Assault Rating.
        let defender_toe: u32 = defenders
            .iter()
            .filter_map(|&id| self.units.get(&id))
            .map(|u| u.total_toe())
            .sum();
        let zero_def_loner = defenders.len() == 1 && defender_toe > 0 && d_raw_troops == 0;

        // Morale (§15.6 / §17.2). §17.26 "enemy 3× strength" uses the opponent's raw
        // offensive vs this side's raw defensive.
        let a_raw_def = self.close_assault_raw(attackers, false);
        let d_raw_off = self.close_assault_raw(defenders, true);
        let (a_basic, a_coh) = self.side_morale_inputs(attackers);
        let (d_basic, d_coh) = self.side_morale_inputs(defenders);
        let attacker_enemy_3x = d_raw_off >= 3 * a_raw_def;
        let defender_enemy_3x = a_raw_off >= 3 * d_raw_troops;
        let attacker_rommel = self.side_has_rommel(attackers);
        let defender_rommel = self.side_has_rommel(defenders);
        let attacker_morale =
            self.adjusted_morale(a_basic, a_coh, attacker_morale_read, attacker_enemy_3x, attacker_rommel);
        let defender_morale =
            self.adjusted_morale(d_basic, d_coh, defender_morale_read, defender_enemy_3x, defender_rommel);

        let mut out = CloseAssaultOutcome {
            differential,
            attacker_strength,
            defender_strength,
            attacker_combined_arms,
            defender_combined_arms,
            zero_def_loner,
            base_column,
            morale_shift: 0,
            size_shift: 0,
            terrain_shift: 0,
            adjusted_column: base_column,
            attacker_morale,
            defender_morale,
            attacker_read,
            defender_read,
            attacker: NO_RESULT,
            defender: NO_RESULT,
            is_overrun: false,
            attacker_toe_removed: 0,
            defender_toe_removed: 0,
            defender_vulnerability_removed: 0,
            attacker_prisoners: 0,
            defender_prisoners: 0,
            attacker_prisoner_roll: None,
            defender_prisoner_roll: None,
        };

        // Surrender (§17.25). Both surrender → ignored, no assault, both Engaged.
        if attacker_morale.surrendered && defender_morale.surrendered {
            out.attacker_morale.surrendered = false;
            out.defender_morale.surrendered = false;
            self.set_status(attackers, |s| s.engaged = true);
            self.set_status(defenders, |s| s.engaged = true);
            out.attacker.engaged = true;
            out.defender.engaged = true;
            return out;
        }
        if attacker_morale.surrendered || defender_morale.surrendered {
            // The surrendering side's Strength Points are lost / become prisoners.
            let (side, other) = if attacker_morale.surrendered {
                (attackers, defenders)
            } else {
                (defenders, attackers)
            };
            let lost = self.surrender_units(side);
            if attacker_morale.surrendered {
                out.attacker_toe_removed = lost;
                out.attacker_prisoners = lost;
            } else {
                out.defender_toe_removed = lost;
                out.defender_prisoners = lost;
            }
            self.set_status(other, |s| s.in_contact = true);
            return out;
        }

        // Column shifts (§15.27): morale (§15.6), unit size (§15.5), terrain (§15.3).
        let morale_shift = attacker_morale.adjusted - defender_morale.adjusted;
        let size_shift = if zero_def_loner {
            // §15.54: §15.51 does not apply; +3 columns for the attacker, plus §15.53.
            org_size_shift(
                self.side_largest_org(attackers),
                self.side_largest_org(defenders),
            ) + 3
        } else {
            self.close_assault_size_shift(attackers, defenders)
        };
        let terrain_shift = self.close_assault_terrain_shift(attackers, defenders);
        let env_shift = self.close_assault_env_shift(attackers, defenders);
        let total_shift = morale_shift + size_shift + terrain_shift + env_shift;
        let adjusted_column = (base_column as i32 + total_shift).clamp(0, 17) as usize;
        out.morale_shift = morale_shift;
        out.size_shift = size_shift;
        out.terrain_shift = terrain_shift;
        out.adjusted_column = adjusted_column;
        out.is_overrun = crate::reference::crt::CloseAssaultCrt::is_overrun_column(adjusted_column);

        let (attacker, defender) = match self.reference.close_assault_crt.as_ref() {
            Some(crt) => (
                crt.resolve_at_column(
                    CloseAssaultSide::Attacker,
                    adjusted_column,
                    attacker_read,
                    dice_sum(attacker_read),
                ),
                crt.resolve_at_column(
                    CloseAssaultSide::Defender,
                    adjusted_column,
                    defender_read,
                    dice_sum(defender_read),
                ),
            ),
            None => (NO_RESULT, NO_RESULT),
        };
        out.attacker = attacker;
        out.defender = defender;

        out.attacker_toe_removed = if zero_def_loner {
            0 // §15.54: the attacking unit takes no casualties.
        } else {
            // §15.83c: the attacker rounds raw points lost up.
            self.apply_raw_loss(attackers, attacker.loss_percent, true, true)
        };
        out.defender_toe_removed = {
            // §15.21/§15.84a: only melee troops take Close Assault losses outside an Overrun;
            // in an Overrun the guns are part of the loss total and absorb losses too.
            let participating: Vec<UnitId> = if out.is_overrun {
                defenders.to_vec()
            } else {
                def_troop_ids.clone()
            };
            // §15.83c: defender rounds raw points lost down (§15.77: up in an Overrun).
            let mut removed = self.apply_raw_loss(&participating, defender.loss_percent, false, out.is_overrun);
            // §15.84b/c: Vulnerability losses to guns — Forward guns always, all guns in an
            // Overrun (AA/Flak exempt) — ≥50% as many Vulnerability Points as Raw Strength
            // Points lost, ≥1 if any, taken after % losses.
            let total_raw = self.close_assault_raw(&participating, false);
            let raw_lost = {
                let s = total_raw * defender.loss_percent as i32;
                if out.is_overrun {
                    (s + 99) / 100
                } else {
                    s / 100
                }
            };
            if raw_lost > 0 {
                let affected: Vec<UnitId> = def_gun_ids
                    .iter()
                    .copied()
                    .filter(|&id| {
                        out.is_overrun
                            || self.units.get(&id).map(|u| u.gun_position == GunPosition::Forward).unwrap_or(false)
                    })
                    .collect();
                let vuln_damage = ((raw_lost + 1) / 2).max(1);
                let v = self.apply_vulnerability_damage(&affected, vuln_damage);
                out.defender_vulnerability_removed = v;
                removed += v;
            }
            removed
        };

        // Prisoners (§15.89): a Capt result converts a fraction of the *losing* side's
        // casualties into prisoners of the opposing Player (a subset of the casualties).
        let (dp, dpr) = self.roll_prisoners(defender.captured, out.defender_toe_removed);
        out.defender_prisoners = dp;
        out.defender_prisoner_roll = dpr;
        let (ap, apr) = self.roll_prisoners(attacker.captured, out.attacker_toe_removed);
        out.attacker_prisoners = ap;
        out.attacker_prisoner_roll = apr;

        // Status effects. Eng (attacker block) makes the defender Engaged; otherwise a
        // side's In-Contact result marks its units In Contact. (A defender Retreat
        // requirement is carried on `defender.retreat_hexes` for the owning player to
        // satisfy — retreat movement is a follow-on decision, not auto-applied here.)
        if attacker.engaged {
            self.set_status(defenders, |s| s.engaged = true);
        }
        if attacker.in_contact {
            self.set_status(attackers, |s| s.in_contact = true);
        }
        if defender.in_contact {
            self.set_status(defenders, |s| s.in_contact = true);
        }
        // §15.82: record a mandated Retreat on each on-map defender counter (the owning
        // player satisfies it with a Retreat command; attached sub-units ride the parent).
        if defender.retreat_hexes > 0 {
            for &d in defenders {
                if let Some(loc) = self.units.get(&d).and_then(|u| u.hex()) {
                    let u = self.units.get_mut(&d).expect("checked");
                    u.pending_retreat = defender.retreat_hexes;
                    u.retreat_origin = Some(loc);
                }
            }
        }
        out
    }

    /// Representative (Basic Morale, Cohesion) for a side (§6.27 / §15.63 / §15.64c): take
    /// the largest units (by stacking points) and average their Basic Morale and Cohesion
    /// (rounded to nearest), covering the multi-division / multi-brigade case.
    fn side_morale_inputs(&self, units: &[UnitId]) -> (i32, i32) {
        let max_sp = units
            .iter()
            .filter_map(|id| self.units.get(id))
            .map(|u| u.stacking_points())
            .max();
        let Some(max_sp) = max_sp else {
            return (0, 0);
        };
        let largest: Vec<_> = units
            .iter()
            .filter_map(|id| self.units.get(id))
            .filter(|u| u.stacking_points() == max_sp)
            .collect();
        let n = largest.len() as i32;
        let basic = round_div(largest.iter().map(|u| u.basic_morale).sum(), n);
        let cohesion = round_div(largest.iter().map(|u| u.cohesion).sum(), n);
        (basic, cohesion)
    }

    /// Adjusted Morale Rating for one side (§17.2): roll the Morale Modifier Table at the
    /// side's Cohesion Level, apply §17.23 clamp [-3, +3], §17.26 Surrender exception, and
    /// the §17.28 Rommel +1 (applied *after* the clamp, so it may reach +4).
    fn adjusted_morale(
        &self,
        basic: i32,
        cohesion: i32,
        roll: u8,
        enemy_3x: bool,
        rommel: bool,
    ) -> SideMorale {
        let bonus = if rommel { 1 } else { 0 };
        let modifier = self
            .reference
            .morale_modifier_table
            .as_ref()
            .map(|t| t.modifier(cohesion, roll))
            .unwrap_or(MoraleModifier::Delta(0));
        match modifier {
            MoraleModifier::Delta(n) => SideMorale {
                basic,
                cohesion,
                roll,
                modifier,
                adjusted: (basic + n).clamp(-3, 3) + bonus,
                surrendered: false,
            },
            MoraleModifier::Surrender => {
                let ignores = basic >= 1 && !(cohesion <= -11 || enemy_3x);
                SideMorale {
                    basic,
                    cohesion,
                    roll,
                    modifier,
                    adjusted: if ignores { (basic - 4).clamp(-3, 3) + bonus } else { 0 },
                    surrendered: !ignores,
                }
            }
        }
    }

    /// Whether a side has any unit stacked with General Rommel (§31.3 / §17.28) — either the Rommel
    /// counter is present-on-map (§31) in a participating unit's hex, or a unit carries the explicit
    /// `with_rommel` marker.
    fn side_has_rommel(&self, units: &[UnitId]) -> bool {
        let rommel_hex = self.rommel_hex();
        units
            .iter()
            .filter_map(|id| self.units.get(id))
            .any(|u| u.status.with_rommel || (rommel_hex.is_some() && u.hex() == rommel_hex))
    }

    /// Unit-size column shift (§15.5): §15.51 double-strength (+2 to the side with ≥2× the
    /// other's Raw CA points) plus §15.53 organizational-size difference, positive toward
    /// the attacker. (§15.54's zero-defensive-CA loner override is applied by the resolver.)
    pub fn close_assault_size_shift(&self, attackers: &[UnitId], defenders: &[UnitId]) -> i32 {
        let a_raw = self.close_assault_raw(attackers, true);
        let d_raw = self.close_assault_raw(defenders, false);
        let mut shift = 0;
        // §15.51: at least twice the Raw CA points -> +2 columns to the superior side.
        if a_raw > 0 && a_raw >= 2 * d_raw {
            shift += 2;
        } else if d_raw > 0 && d_raw >= 2 * a_raw {
            shift -= 2;
        }
        // §15.53: organizational-size difference (largest unit each side).
        shift += org_size_shift(
            self.side_largest_org(attackers),
            self.side_largest_org(defenders),
        );
        shift
    }

    /// Terrain column shift (§15.3): the defender-hex terrain (§15.32) plus the hexside
    /// crossed into the defender's hex (§15.33). Per §15.35 the defender benefits from only
    /// one *type* of hexside — so among distinct attacking hexes we apply the single most
    /// defender-favorable edge — **except** when one crossed edge itself carries two terrain
    /// features (the rare §15.35 "wadi-along-slope" case, e.g. B5810–B5809), where both apply:
    /// `hexsides_on` returns every feature on an edge and we sum them before picking the best edge.
    /// Signs follow the chart (right = attacker favor). Uses each unit's effective hex; if the
    /// defender has no location, the shift is 0. (Close Assault always targets one hex — the whole
    /// enemy stack there, §15.23 — so there are no multi-hex defenders to reconcile.)
    /// §23.24 engineer-vs-fortification and §26.26 enemy-minefield close-assault column shifts, added
    /// to the §15.27 total (positive = attacker's favor). §23.24: an unpinned Engineer (or engineer
    /// battalion / Pioneer) among the assaulting units, against a fortified *non-major-city* hex,
    /// shifts +1 to the attacker. §26.26: if the assaulting units stand in — or the assaulted hex is —
    /// an enemy (real) minefield, the defender adjusts one column in his favor (−1 here).
    pub(crate) fn close_assault_env_shift(&self, attackers: &[UnitId], defenders: &[UnitId]) -> i32 {
        use crate::reference::terrain::HexTerrain;
        let Some(def_hex) = defenders.iter().find_map(|&id| self.effective_hex(id)) else {
            return 0;
        };
        let atk_side = attackers.iter().find_map(|&a| self.units.get(&a)).map(|u| u.nationality.side());
        let mut shift = 0;
        // §23.24: engineer attached to the assault vs a fortified (non-major-city) hex.
        if self.fort_level(def_hex) > 0 && self.map.terrain(def_hex) != Some(HexTerrain::MajorCity) {
            let has_engineer = attackers.iter().any(|&a| {
                self.units.get(&a).is_some_and(|u| {
                    let ut = self
                        .reference
                        .unit_characteristics
                        .get(&u.nationality)
                        .and_then(|m| m.get(&u.id_code))
                        .map(|c| c.unit_type.as_str())
                        .unwrap_or("");
                    // Engineer-capable per the construction check (§24.24): an Engineer unit, an
                    // engineer-capable HQ, or the Australian Pioneer battalion (§20.3 footnote e).
                    (ut.contains("Engineer") || ut.contains("Headquarters") || u.name.contains("Pioneer"))
                        && !u.status.pinned
                })
            });
            if has_engineer {
                shift += 1;
            }
        }
        // §26.26: enemy minefield under the attackers or the defenders.
        if let Some(side) = atk_side {
            let enemy_mine = |h: HexCoord| self.minefields.get(&h).is_some_and(|m| !m.dummy && m.side != side);
            let atk_in_mine = attackers.iter().filter_map(|&a| self.effective_hex(a)).any(enemy_mine);
            if atk_in_mine || enemy_mine(def_hex) {
                shift -= 1;
            }
        }
        shift
    }

    fn close_assault_terrain_shift(&self, attackers: &[UnitId], defenders: &[UnitId]) -> i32 {
        let Some(def_hex) = defenders.iter().find_map(|&id| self.effective_hex(id)) else {
            return 0;
        };
        let mut shift = 0;

        // §15.32: terrain of the defended hex.
        if let Some(t) = self.map.terrain(def_hex) {
            if let Some(p) = self.reference.terrain.get(&t) {
                shift += combat_mod_shift(p.close_assault);
            }
        }
        // §15.35: fortifications are cumulative with terrain (and hexsides) in close assault.
        shift += self.fort_shift(def_hex);

        // §15.33 / §15.35: the single most defender-favorable hexside crossed.
        let mut attacker_hexes: Vec<HexCoord> =
            attackers.iter().filter_map(|&id| self.effective_hex(id)).collect();
        attacker_hexes.sort_by_key(|h| (h.col, h.row));
        attacker_hexes.dedup();
        let mut best: Option<i32> = None;
        for ah in attacker_hexes {
            let sides = self.map.hexsides_on(ah, def_hex);
            if sides.is_empty() {
                continue;
            }
            // Sum the close-assault shifts of every feature on this edge, then keep the best
            // (lowest) across the attacker's hexes.
            let s: i32 = sides
                .iter()
                .filter_map(|hs| self.reference.hexsides.get(&hs.resolve(ah, def_hex)))
                .map(|p| combat_mod_shift(p.close_assault))
                .sum();
            best = Some(best.map_or(s, |b| b.min(s)));
        }
        if let Some(s) = best {
            shift += s;
        }
        shift
    }

    /// The largest effective organizational size among a set of units (largest-unit rule,
    /// shell-reduced per §9.28).
    fn side_largest_org(&self, units: &[UnitId]) -> OrgSize {
        units
            .iter()
            .map(|&id| self.effective_org_size(id))
            .max_by_key(|o| o.stacking_points())
            .unwrap_or(OrgSize::Battalion)
    }

    /// A side Surrenders: eliminate every unit's Strength Points and flag it; return the
    /// total TOE lost (all taken prisoner).
    pub(crate) fn surrender_units(&mut self, units: &[UnitId]) -> u32 {
        let mut lost = 0;
        for &id in units {
            // §28.11/§28.3: infantry TOE → Prisoner Points, weapons TOE → captured equipment, taken
            // by the opponent — recorded before the unit's Strength is zeroed.
            self.capture_on_surrender(id);
            if let Some(u) = self.units.get_mut(&id) {
                lost += u.total_toe();
                match &mut u.composition {
                    Composition::Organic { current_toe } => *current_toe = 0,
                    Composition::Weapons { groups } => {
                        for g in groups.iter_mut() {
                            g.count = 0;
                        }
                    }
                }
                u.status.surrendered = true;
            }
        }
        lost
    }

    /// Roll the Prisoners table (§15.89) if `captured`, returning (prisoners, die). The
    /// prisoners are `percent`% (round half up) of `casualties`.
    fn roll_prisoners(&mut self, captured: bool, casualties: u32) -> (u32, Option<u8>) {
        if !captured || casualties == 0 {
            return (0, None);
        }
        let die = self.rng.d6();
        let pct = self
            .reference
            .prisoners_table
            .as_ref()
            .map(|t| t.percent(die))
            .unwrap_or(0);
        let prisoners = (casualties * pct as u32 + 50) / 100;
        (prisoners, Some(die))
    }

    /// Apply a status mutation to every unit in a set.
    fn set_status(&mut self, units: &[UnitId], f: impl Fn(&mut crate::state::unit::UnitStatus)) {
        for &id in units {
            if let Some(u) = self.units.get_mut(&id) {
                f(&mut u.status);
            }
        }
    }

    /// §15.83 percentage losses for a side: raw Close Assault points lost = the CRT
    /// percentage × the side's total Raw Close Assault strength, rounded up if `round_up`
    /// (attacker, §15.83c; and Overrun defender, §15.77) else down (defender). Then remove
    /// TOE to absorb those raw points — each removed TOE absorbs its own Close Assault
    /// rating (§15.83d) — processing units in the order given (a default the caller may
    /// order to control which units bleed first). Returns TOE removed.
    pub(crate) fn apply_raw_loss(&mut self, units: &[UnitId], percent: u8, offensive: bool, round_up: bool) -> u32 {
        if percent == 0 {
            return 0;
        }
        let total_raw = self.close_assault_raw(units, offensive);
        if total_raw <= 0 {
            return 0;
        }
        let scaled = total_raw * percent as i32;
        let mut remaining = if round_up {
            (scaled + 99) / 100
        } else {
            scaled / 100
        };
        if remaining <= 0 {
            return 0;
        }
        let kind = if offensive {
            RatingKind::CloseAssaultOff
        } else {
            RatingKind::CloseAssaultDef
        };
        // Per-element Close Assault rating (raw absorbed per TOE point).
        let mut elems: Vec<(UnitId, Option<usize>, i32)> = Vec::new();
        for &id in units {
            let Some(u) = self.units.get(&id) else {
                continue;
            };
            match &u.composition {
                Composition::Organic { current_toe } => {
                    let raw = u.raw_strength(&self.reference, kind);
                    if *current_toe > 0 && raw > 0 {
                        elems.push((id, None, raw / (*current_toe as i32)));
                    }
                }
                Composition::Weapons { groups } => {
                    let table = self.reference.weapons_systems.get(&u.nationality);
                    for (gi, g) in groups.iter().enumerate() {
                        let ca = table
                            .and_then(|m| m.get(&g.system))
                            .and_then(|ws| ws.close_assault)
                            .map(|c| if offensive { c.off.value } else { c.def.value })
                            .unwrap_or(0);
                        if ca > 0 && g.count > 0 {
                            elems.push((id, Some(gi), ca));
                        }
                    }
                }
            }
        }
        let mut removed = 0u32;
        for (id, grp, ca) in elems {
            if remaining <= 0 {
                break;
            }
            let count = match &self.units.get(&id).unwrap().composition {
                Composition::Organic { current_toe } => *current_toe,
                Composition::Weapons { groups } => grp.map(|gi| groups[gi].count).unwrap_or(0),
            };
            if count == 0 {
                continue;
            }
            let take = (((remaining + ca - 1) / ca) as u32).min(count);
            if let Some(u) = self.units.get_mut(&id) {
                match &mut u.composition {
                    Composition::Organic { current_toe } => *current_toe -= take,
                    Composition::Weapons { groups } => {
                        if let Some(gi) = grp {
                            groups[gi].count -= take;
                        }
                    }
                }
            }
            removed += take;
            remaining -= (take as i32) * ca;
        }
        removed
    }
}

/// The outcome of one morale check on the Morale Modifier Table (§17.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoraleCheckOutcome {
    pub roll: u8,
    pub modifier: MoraleModifier,
    /// The Basic-Morale adjustment for that combat (0 on Surrender).
    pub morale_delta: i32,
    /// The unit Surrendered (all Strength Points lost).
    pub surrendered: bool,
}

impl GameState {
    /// Perform a morale check for a unit at a given Disorganization Level (§17.4), rolling
    /// two dice. Returns the Basic-Morale adjustment "for that combat" (which the caller
    /// applies) or, on Surrender, eliminates the unit's Strength Points and flags it.
    ///
    /// The Disorganization Level input is supplied by the caller: how it derives from the
    /// unit's tracked cohesion, and where in the close-assault sequence this check fires,
    /// are §15/§17 sequencing rules wired in separately.
    pub fn morale_check(&mut self, unit: UnitId, disorganization_level: i32) -> MoraleCheckOutcome {
        let roll = self.rng.two_dice_raw();
        self.morale_check_with_dice(unit, disorganization_level, roll)
    }

    /// Morale check with an explicit two-dice read (11..66).
    pub fn morale_check_with_dice(
        &mut self,
        unit: UnitId,
        disorganization_level: i32,
        roll: u8,
    ) -> MoraleCheckOutcome {
        let modifier = self
            .reference
            .morale_modifier_table
            .as_ref()
            .map(|t| t.modifier(disorganization_level, roll))
            .unwrap_or(MoraleModifier::Delta(0));

        match modifier {
            MoraleModifier::Delta(n) => MoraleCheckOutcome {
                roll,
                modifier,
                morale_delta: n,
                surrendered: false,
            },
            MoraleModifier::Surrender => {
                // §28.11/§28.3: capture the surrendering Strength before it is zeroed.
                self.capture_on_surrender(unit);
                if let Some(u) = self.units.get_mut(&unit) {
                    match &mut u.composition {
                        Composition::Organic { current_toe } => *current_toe = 0,
                        Composition::Weapons { groups } => {
                            for g in groups.iter_mut() {
                                g.count = 0;
                            }
                        }
                    }
                    u.status.surrendered = true;
                }
                MoraleCheckOutcome {
                    roll,
                    modifier,
                    morale_delta: 0,
                    surrendered: true,
                }
            }
        }
    }
}
