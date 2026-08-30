//! Per-player observation layer (§3.5 hidden information / D5).
//!
//! CNA leans hard on secrecy: a player sees the enemy's *counters* on the map but not their current
//! TOE Strength, cannot tell a real Supply Dump from a dummy, and never sees a hidden desert raider
//! until it raids or is spotted (§27.32/§27.33). The core `GameState` holds ground truth; this
//! module derives a *thin* per-side view of what a player is entitled to know, so an AI agent (or a
//! UI) acts on knowledge rather than omniscience. Nothing here mutates state.

use crate::reference::units::{Nationality, Side};
use crate::state::game::{Commodities, GameState, RaiderKind};
use crate::state::map::HexCoord;
use crate::state::unit::{OrgSize, UnitId, UnitLocation};

/// What a viewing side knows about a unit (§3.5): the counter is visible (type, size, hex), but its
/// current TOE `strength` is secret unless the unit is the viewer's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitView {
    pub id: UnitId,
    pub name: String,
    pub nationality: Nationality,
    pub org_size: OrgSize,
    pub is_hq: bool,
    pub hex: Option<HexCoord>,
    /// Current TOE Strength Points — `Some` only for the viewer's own units (§3.5 secrecy).
    pub strength: Option<u32>,
    /// Whether this is one of the viewer's own units.
    pub friendly: bool,
}

/// What a viewing side knows about a Supply Dump: a friendly dump shows its side, dummy status, and
/// contents; an enemy dump shows only that a dump marker is present (§32.18 — real vs dummy and the
/// stored commodities are hidden until raided/overrun).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpView {
    pub location: UnitLocation,
    pub side: Side,
    pub friendly: bool,
    /// Contents — `Some` only for a friendly dump.
    pub contents: Option<Commodities>,
    /// Dummy flag — `Some` only for a friendly dump (an enemy cannot tell).
    pub dummy: Option<bool>,
}

/// What a viewing side knows about a desert raider: its own raiders always; enemy raiders only once
/// revealed (§27.32/§27.33 — a hidden raider is off the map to the enemy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaiderView {
    pub kind: RaiderKind,
    pub side: Side,
    pub hex: Option<HexCoord>,
    pub friendly: bool,
}

/// What a viewing side knows about a minefield (§26): the marker's hex and owner are visible, but
/// whether it is real or a dummy is hidden until it is the viewer's own or has been revealed by an
/// enemy unit entering it (§26.15/§26.23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinefieldView {
    pub hex: HexCoord,
    pub side: Side,
    pub friendly: bool,
    /// Dummy flag — `Some` only when the viewer owns the field or it has been revealed.
    pub dummy: Option<bool>,
}

impl GameState {
    /// §3.5: the units `viewer` can observe — every on-map counter (its own with full TOE Strength,
    /// the enemy's with `strength = None`). Off-map units (holding boxes) are included like the map.
    pub fn observe_units(&self, viewer: Side) -> Vec<UnitView> {
        self.units
            .values()
            .map(|u| {
                let friendly = u.nationality.side() == viewer;
                UnitView {
                    id: u.id,
                    name: u.name.clone(),
                    nationality: u.nationality,
                    org_size: u.org_size,
                    is_hq: u.is_hq,
                    hex: u.hex(),
                    strength: friendly.then(|| u.total_toe()),
                    friendly,
                }
            })
            .collect()
    }

    /// §32.18: the Supply Dumps `viewer` can observe — friendly dumps in full; enemy dumps only as a
    /// present marker (contents and dummy status hidden).
    pub fn observe_supply_dumps(&self, viewer: Side) -> Vec<DumpView> {
        self.supply_dumps
            .iter()
            .map(|d| {
                let friendly = d.side == viewer;
                DumpView {
                    location: d.location.clone(),
                    side: d.side,
                    friendly,
                    contents: friendly.then_some(d.supply),
                    dummy: friendly.then_some(d.dummy),
                }
            })
            .collect()
    }

    /// §27.32/§27.33: the desert raiders `viewer` can observe — its own (wherever they are) plus any
    /// enemy raider that has been revealed by raiding or being spotted.
    pub fn observe_raiders(&self, viewer: Side) -> Vec<RaiderView> {
        self.raiders
            .iter()
            .filter(|r| r.hex.is_some())
            .filter(|r| r.side == viewer || r.revealed)
            .map(|r| RaiderView {
                kind: r.kind,
                side: r.side,
                hex: r.hex,
                friendly: r.side == viewer,
            })
            .collect()
    }

    /// §26: the minefields `viewer` can observe — the marker and owner of each, with its real/dummy
    /// status disclosed only for the viewer's own fields or ones already revealed (§26.15/§26.23).
    pub fn observe_minefields(&self, viewer: Side) -> Vec<MinefieldView> {
        self.minefields
            .iter()
            .map(|(&hex, mf)| {
                let known = mf.side == viewer || self.revealed_minefields.contains(&hex);
                MinefieldView {
                    hex,
                    side: mf.side,
                    friendly: mf.side == viewer,
                    dummy: known.then_some(mf.dummy),
                }
            })
            .collect()
    }
}
