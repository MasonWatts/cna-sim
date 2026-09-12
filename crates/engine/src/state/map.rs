//! The game map: hex coordinates and the hex container.
//!
//! Coordinates are exposed as CNA-style offset `(col, row)` (matching hex labels like
//! `B5810` → col 58, row 10). Adjacency/distance math uses cube coordinates internally
//! to stay parity-bug-free. Layout: **pointy-top**, **even-r** vertical offset — hexes in
//! a row (RR) share vertical edges; confirmed against the section-C map import (4021
//! Sollum borders 4022 Sea).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;

use crate::reference::terrain::{HexTerrain, HexsideTerrain};

/// A hex position in CNA offset coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HexCoord {
    pub col: i16,
    pub row: i16,
}

/// Column span reserved per map section (A–E) when placing them in one global column
/// space. Sections physically overlap at their seams (§4.1); this large span keeps
/// different sections from colliding (at the cost of gaps between them). Cross-section
/// seam merging is a later refinement — within one section this is an identity on col.
///
/// The five map sheets are each 34 columns wide and overlap by one column at each seam, laid out
/// west→east A–E. So each section starts `SECTION_ADVANCE` (33) columns after the previous — the
/// shared seam column resolves to a single coordinate, and load order A→E keeps the right sheet's
/// copy (§8.71 map layout). The sheets are also slightly staggered vertically (`SECTION_ROW_OFFSET`).
const SECTION_ADVANCE: i32 = 33;
// One continuous pointy-top even-r grid: no per-section row offset. An *odd* offset would flip the
// even-r stagger parity of the shifted sheets, misaligning the grid at the seam (the "fingers" that
// appeared from C eastward under the old [0,0,1,1,1]). Any apparent vertical stagger between sheets
// is just the inherent even-r row stagger, already encoded by row parity — not an extra offset.
const SECTION_ROW_OFFSET: [i32; 5] = [0, 0, 0, 0, 0]; // A, B, C, D, E

/// The map-section letter (`A`–`E`) for a global column (weather is resolved per section, §29.1).
pub fn section_char_of_col(col: i16) -> char {
    (b'A' + ((col as i32 - 1).max(0) / SECTION_ADVANCE) as u8) as char
}

/// A CNA map-hex identifier: a section letter (`A`–`E`) plus the printed 4-digit label
/// `RRCC` — `RR` = row, `CC` = column (pointy-top grid: hexes in a row share vertical
/// edges; adjacent rows are half-staggered). E.g. `C0101`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HexId {
    pub section: char,
    pub rr: u16,
    pub cc: u16,
}

impl HexId {
    /// Parse a label like `"C0101"`: a section letter A–E followed by exactly four digits
    /// (two-digit `RR`, two-digit `CC`). Returns `None` if malformed.
    pub fn parse(s: &str) -> Option<HexId> {
        let s = s.trim();
        let mut chars = s.chars();
        let section = chars.next()?.to_ascii_uppercase();
        if !('A'..='E').contains(&section) {
            return None;
        }
        let digits = chars.as_str();
        if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let rr = digits[0..2].parse().ok()?;
        let cc = digits[2..4].parse().ok()?;
        Some(HexId { section, rr, cc })
    }

    fn section_index(&self) -> i32 {
        (self.section as u8 - b'A') as i32
    }

    /// Convert to the internal `HexCoord`: `CC` is the column and `RR` the row of a
    /// pointy-top grid; the section is offset into a global column space so sections
    /// don't collide.
    pub fn to_hex_coord(&self) -> HexCoord {
        let idx = self.section_index();
        let col = idx * SECTION_ADVANCE + self.cc as i32;
        let row = SECTION_ROW_OFFSET.get(idx as usize).copied().unwrap_or(0) + self.rr as i32;
        HexCoord::new(col as i16, row as i16)
    }
}

impl std::fmt::Display for HexId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{:02}{:02}", self.section, self.rr, self.cc)
    }
}

impl std::str::FromStr for HexId {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        HexId::parse(s).ok_or(())
    }
}

/// Internal cube coordinate (x + y + z == 0) used for hex math.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cube {
    x: i32,
    y: i32,
    z: i32,
}

impl HexCoord {
    pub const fn new(col: i16, row: i16) -> Self {
        Self { col, row }
    }

    fn to_cube(self) -> Cube {
        let col = self.col as i32;
        let row = self.row as i32;
        // Pointy-top, even-r offset -> cube. The CNA map is pointy-top: hexes in the same
        // row (RR) share vertical edges (E/W neighbors); adjacent rows are half-staggered.
        let z = row;
        let x = col - (row + (row & 1)) / 2;
        let y = -x - z;
        Cube { x, y, z }
    }

    fn from_cube(c: Cube) -> Self {
        let row = c.z;
        let col = c.x + (c.z + (c.z & 1)) / 2;
        Self {
            col: col as i16,
            row: row as i16,
        }
    }

    /// The six adjacent hex coordinates (pointy-top, even-r layout), in a fixed order.
    pub fn neighbors(self) -> [HexCoord; 6] {
        const DIRS: [(i32, i32, i32); 6] = [
            (1, -1, 0),
            (1, 0, -1),
            (0, 1, -1),
            (-1, 1, 0),
            (-1, 0, 1),
            (0, -1, 1),
        ];
        let c = self.to_cube();
        let mut out = [self; 6];
        for (i, (dx, dy, dz)) in DIRS.iter().enumerate() {
            out[i] = HexCoord::from_cube(Cube {
                x: c.x + dx,
                y: c.y + dy,
                z: c.z + dz,
            });
        }
        out
    }

    /// Hex distance (number of steps) between two coordinates.
    pub fn distance(self, other: HexCoord) -> u32 {
        let a = self.to_cube();
        let b = other.to_cube();
        (((a.x - b.x).abs() + (a.y - b.y).abs() + (a.z - b.z).abs()) / 2) as u32
    }

    pub fn is_adjacent(self, other: HexCoord) -> bool {
        self.distance(other) == 1
    }
}

/// A point feature layered on top of a hex's base terrain. Features don't affect
/// movement (§8.48); they matter for supply, water, victory points, garrisons and ports,
/// and are distinct from the base `HexTerrain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feature {
    /// Nondiminishing Water + Stores supply dump (§8.48).
    Oasis,
    /// A settlement: has a well (water), and counts for VP / supply / garrison / forts.
    Village,
    /// A named well — a water source (§52.16) like a village, but not a VP / supply /
    /// garrison location.
    Bir,
    /// A logistics embark/disembark facility on a coastal hex.
    Port,
}

impl Feature {
    /// Whether this feature is a water source (§8.48 oasis; §52.16 village/bir wells).
    pub fn is_water_source(self) -> bool {
        matches!(self, Feature::Oasis | Feature::Village | Feature::Bir)
    }
}

/// A single hex's contents: base terrain plus any point features on top of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hex {
    pub terrain: HexTerrain,
    #[serde(default)]
    pub features: Vec<Feature>,
    /// The place name printed on the paper map (a Village/Major-City name). Cosmetic — used only
    /// for the viewer's map labels. `None` when unnamed or not yet transcribed. Birs are labeled a
    /// generic "Bir" by the viewer and don't need a stored name.
    #[serde(default)]
    pub label: Option<String>,
}

impl Hex {
    pub fn has_feature(&self, f: Feature) -> bool {
        self.features.contains(&f)
    }
    /// Any water-source feature present (oasis / village / bir).
    pub fn has_water_source(&self) -> bool {
        self.features.iter().any(|f| f.is_water_source())
    }
    pub fn is_port(&self) -> bool {
        self.has_feature(Feature::Port)
    }
}

/// The base kind of a hexside feature as stored on the map. Directional kinds (Slope,
/// Escarpment) resolve to the reference's Up/Down variants using `high_side`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HexsideKind {
    Wadi,
    Ridge,
    Slope,
    Escarpment,
    MajorRiver,
    MinorRiver,
    Road,
    Track,
    Railroad,
    /// An incomplete stretch of road (§24.5): treated as a Track for all purposes (§24.52) until
    /// built into a real Road by construction (§24.53).
    UnfinishedRoad,
    /// Unlaid track on a planned rail line (§24.6): serves no function at all (§24.67) until built
    /// by the NZRRC companies; marks where the railhead may extend.
    UnfinishedRailroad,
    /// Built track that has been destroyed (§24.66): inert until rebuilt by engineers (§24.63).
    DestroyedRailroad,
    Lake,
}

/// A hexside feature on the edge between two hexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hexside {
    pub kind: HexsideKind,
    /// For directional kinds (Slope/Escarpment): which of the two hexes is the "up" side.
    pub high_side: Option<HexCoord>,
}

impl Hexside {
    /// Resolve to the reference `HexsideTerrain` for travel `from -> to` (picks Up/Down
    /// for directional kinds: moving *toward* the high side is "up").
    pub fn resolve(&self, _from: HexCoord, to: HexCoord) -> HexsideTerrain {
        let toward_high = Some(to) == self.high_side;
        match self.kind {
            HexsideKind::Slope => {
                if toward_high {
                    HexsideTerrain::UpSlope
                } else {
                    HexsideTerrain::DownSlope
                }
            }
            HexsideKind::Escarpment => {
                if toward_high {
                    HexsideTerrain::UpEscarpment
                } else {
                    HexsideTerrain::DownEscarpment
                }
            }
            HexsideKind::Wadi => HexsideTerrain::Wadi,
            HexsideKind::Ridge => HexsideTerrain::Ridge,
            HexsideKind::MajorRiver => HexsideTerrain::MajorRiver,
            HexsideKind::MinorRiver => HexsideTerrain::MinorRiver,
            HexsideKind::Road => HexsideTerrain::Road,
            HexsideKind::Track => HexsideTerrain::Track,
            HexsideKind::Railroad => HexsideTerrain::Railroad,
            // §24.52: an unfinished road is a Track until built.
            HexsideKind::UnfinishedRoad => HexsideTerrain::Track,
            // §24.67: unbuilt track serves no function.
            HexsideKind::UnfinishedRailroad => HexsideTerrain::UnfinishedRailroad,
            // §24.66: destroyed track is inert until rebuilt.
            HexsideKind::DestroyedRailroad => HexsideTerrain::UnfinishedRailroad,
            HexsideKind::Lake => HexsideTerrain::Lake,
        }
    }
}

/// Canonical (order-independent) key for the edge between two hexes.
fn edge_key(a: HexCoord, b: HexCoord) -> (HexCoord, HexCoord) {
    if (a.col, a.row) <= (b.col, b.row) {
        (a, b)
    } else {
        (b, a)
    }
}

/// The game map: a sparse grid of hexes plus hexside features on their edges.
///
/// The two collections are held behind `Rc` with copy-on-write in the mutators, so cloning a
/// `Map` (and therefore a whole `GameState`) is O(1) — a refcount bump — until something actually
/// writes to the map. This makes `GameState::dry_run` / the legal-action enumerator cheap: a
/// `MoveStep` or combat command never touches the map, so the probing clone shares it for free.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Map {
    hexes: Rc<HashMap<HexCoord, Hex>>,
    hexsides: Rc<HashMap<(HexCoord, HexCoord), Vec<Hexside>>>,
}

impl Map {
    pub fn new() -> Self {
        Self::default()
    }

    /// Place (or overwrite) a hex with the given base terrain (no features).
    pub fn set_hex(&mut self, coord: HexCoord, terrain: HexTerrain) {
        Rc::make_mut(&mut self.hexes).insert(coord, Hex { terrain, features: Vec::new(), label: None });
    }

    /// Place (or overwrite) a hex with base terrain and point features.
    pub fn set_hex_with_features(
        &mut self,
        coord: HexCoord,
        terrain: HexTerrain,
        features: Vec<Feature>,
    ) {
        Rc::make_mut(&mut self.hexes).insert(coord, Hex { terrain, features, label: None });
    }

    pub fn hex(&self, coord: HexCoord) -> Option<&Hex> {
        self.hexes.get(&coord)
    }

    /// The point features on a hex (empty if the hex is absent or featureless).
    pub fn features(&self, coord: HexCoord) -> &[Feature] {
        self.hexes.get(&coord).map(|h| h.features.as_slice()).unwrap_or(&[])
    }

    /// A land hex adjacent to at least one Sea hex — "coastal" for ports, coastal
    /// shipping, and off-shore shelling. Sea hexes and off-map hexes are not coastal.
    pub fn is_coastal(&self, coord: HexCoord) -> bool {
        match self.terrain(coord) {
            Some(t) if t != HexTerrain::Sea => coord
                .neighbors()
                .iter()
                .any(|n| self.terrain(*n) == Some(HexTerrain::Sea)),
            _ => false,
        }
    }

    /// Load a digitized map section (RON from `tools/map_editor.html`): `RRCC -> (terrain,
    /// features)`. `Invalid` cells are skipped. Returns the number of real hexes loaded.
    pub fn load_section(&mut self, ron_str: &str) -> Result<usize, MapLoadError> {
        let data: SectionData = ron::from_str(ron_str).map_err(MapLoadError::Ron)?;
        let section = data.section.chars().next().unwrap_or('A');
        let mut n = 0;
        for (id, entry) in &data.hexes {
            let Some(terrain) = entry.terrain.to_terrain() else {
                continue; // Invalid (non-hex) — skip
            };
            let full = format!("{section}{id}");
            let hid = HexId::parse(&full).ok_or(MapLoadError::BadId(full))?;
            self.set_hex_with_features(hid.to_hex_coord(), terrain, entry.features.clone());
            n += 1;
        }
        // Edge features (§8.37 hexside terrain). An id is a bare 4-digit RRCC (this section) or a
        // full XRRCC with a section letter (a border edge into an adjacent section).
        let resolve = |id: &str| -> Result<HexCoord, MapLoadError> {
            let full = if id.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                id.to_string()
            } else {
                format!("{section}{id}")
            };
            HexId::parse(&full)
                .map(|h| h.to_hex_coord())
                .ok_or(MapLoadError::BadId(full))
        };
        for hs in &data.hexsides {
            let a = resolve(&hs.a)?;
            let b = resolve(&hs.b)?;
            let high = match &hs.up {
                Some(u) => Some(resolve(u)?),
                None => None,
            };
            self.set_hexside(a, b, hs.kind, high);
        }
        Ok(n)
    }

    pub fn terrain(&self, coord: HexCoord) -> Option<HexTerrain> {
        self.hexes.get(&coord).map(|h| h.terrain)
    }

    pub fn len(&self) -> usize {
        self.hexes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hexes.is_empty()
    }

    /// The adjacent coordinates that actually exist on this map.
    /// All on-map hex coordinates.
    pub fn coords(&self) -> impl Iterator<Item = HexCoord> + '_ {
        self.hexes.keys().copied()
    }

    pub fn neighbors(&self, coord: HexCoord) -> impl Iterator<Item = HexCoord> + '_ {
        coord
            .neighbors()
            .into_iter()
            .filter(move |c| self.hexes.contains_key(c))
    }

    /// Set a hexside feature on the edge between two (adjacent) hexes. `high_side` is the
    /// Set (or add) a hexside feature on the edge between two hexes. Multiple features may share
    /// one edge (e.g. a Road alongside a Railroad, or a Road bridging a River); re-adding the same
    /// `kind` updates its `high_side`. `high_side` is the up-hex for directional kinds
    /// (Slope/Escarpment), and should be `a` or `b`.
    pub fn set_hexside(
        &mut self,
        a: HexCoord,
        b: HexCoord,
        kind: HexsideKind,
        high_side: Option<HexCoord>,
    ) {
        let v = Rc::make_mut(&mut self.hexsides).entry(edge_key(a, b)).or_default();
        if let Some(h) = v.iter_mut().find(|h| h.kind == kind) {
            h.high_side = high_side;
        } else {
            v.push(Hexside { kind, high_side });
        }
    }

    /// All hexside features on the edge between two hexes (empty if none).
    /// Every hexside in the map as `(a, b, &Hexside)` (edge endpoints normalized). For the viewer /
    /// tools that render or scan all edge features.
    pub fn all_hexsides(&self) -> impl Iterator<Item = (HexCoord, HexCoord, &Hexside)> {
        self.hexsides.iter().flat_map(|(&(a, b), v)| v.iter().map(move |h| (a, b, h)))
    }

    pub fn hexsides_on(&self, a: HexCoord, b: HexCoord) -> &[Hexside] {
        self.hexsides.get(&edge_key(a, b)).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Remove every hexside feature of `kind` from the edge between two hexes (e.g. converting an
    /// UnfinishedRoad into a Road via construction). Returns whether anything was removed.
    pub fn remove_hexside(&mut self, a: HexCoord, b: HexCoord, kind: HexsideKind) -> bool {
        let sides = Rc::make_mut(&mut self.hexsides);
        if let Some(v) = sides.get_mut(&edge_key(a, b)) {
            let before = v.len();
            v.retain(|h| h.kind != kind);
            let removed = v.len() != before;
            if v.is_empty() {
                sides.remove(&edge_key(a, b));
            }
            removed
        } else {
            false
        }
    }

    /// The first hexside feature on the edge between two hexes, if any. Convenience for callers
    /// that only need to know a single feature; prefer `hexsides_on` when multiple can matter.
    pub fn hexside(&self, a: HexCoord, b: HexCoord) -> Option<&Hexside> {
        self.hexsides.get(&edge_key(a, b)).and_then(|v| v.first())
    }
}

/// Error loading a map section RON.
#[derive(Debug)]
pub enum MapLoadError {
    Ron(ron::error::SpannedError),
    BadId(String),
}

#[derive(Deserialize)]
struct SectionData {
    section: String,
    hexes: HashMap<String, HexEntry>,
    /// Edge features between hexes (§8.37 hexside terrain). Optional so existing terrain-only
    /// section files still load.
    #[serde(default)]
    hexsides: Vec<HexsideEntry>,
}

/// One hexside/edge entry in a section file. `a`/`b` are hex IDs — a bare 4-digit `RRCC`
/// (this section) or a full `XRRCC` with a section letter (for edges on a section border). `up`,
/// for directional kinds (Slope/Escarpment), names whichever of `a`/`b` is the higher hex.
#[derive(Deserialize)]
struct HexsideEntry {
    a: String,
    b: String,
    kind: HexsideKind,
    #[serde(default)]
    up: Option<String>,
}

#[derive(Deserialize)]
struct HexEntry {
    terrain: LoadTerrain,
    #[serde(default)]
    features: Vec<Feature>,
}

/// Terrain tokens as exported by the editor, including `Invalid` (which is skipped when
/// building the map).
#[derive(Deserialize, Clone, Copy)]
enum LoadTerrain {
    Clear,
    SandGravel,
    SaltMarsh,
    HeavyVegetation,
    Rough,
    Mountain,
    Delta,
    Desert,
    MajorCity,
    Swamp,
    Sea,
    Invalid,
}

impl LoadTerrain {
    fn to_terrain(self) -> Option<HexTerrain> {
        use HexTerrain as T;
        Some(match self {
            LoadTerrain::Clear => T::Clear,
            LoadTerrain::SandGravel => T::SandGravel,
            LoadTerrain::SaltMarsh => T::SaltMarsh,
            LoadTerrain::HeavyVegetation => T::HeavyVegetation,
            LoadTerrain::Rough => T::Rough,
            LoadTerrain::Mountain => T::Mountain,
            LoadTerrain::Delta => T::Delta,
            LoadTerrain::Desert => T::Desert,
            LoadTerrain::MajorCity => T::MajorCity,
            LoadTerrain::Swamp => T::Swamp,
            LoadTerrain::Sea => T::Sea,
            LoadTerrain::Invalid => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_are_all_distance_one_and_distinct() {
        let a = HexCoord::new(10, 10);
        let ns = a.neighbors();
        for n in ns {
            assert_eq!(a.distance(n), 1);
            assert!(a.is_adjacent(n));
        }
        for i in 0..6 {
            for j in (i + 1)..6 {
                assert_ne!(ns[i], ns[j], "neighbors {i} and {j} coincide");
            }
        }
    }

    #[test]
    fn distance_is_symmetric_and_zero_to_self() {
        let a = HexCoord::new(3, 7);
        let b = HexCoord::new(9, 4);
        assert_eq!(a.distance(a), 0);
        assert_eq!(a.distance(b), b.distance(a));
        // A neighbor of a neighbor is at most distance 2.
        let n = a.neighbors()[0];
        let nn = n.neighbors()[0];
        assert!(a.distance(nn) <= 2);
    }

    #[test]
    fn cube_round_trip_including_negative_columns() {
        for col in -5..6 {
            for row in -5..6 {
                let c = HexCoord::new(col, row);
                assert_eq!(HexCoord::from_cube(c.to_cube()), c);
            }
        }
    }

    #[test]
    fn map_terrain_and_on_map_neighbors() {
        let mut m = Map::new();
        m.set_hex(HexCoord::new(0, 0), HexTerrain::Clear);
        m.set_hex(HexCoord::new(1, 0), HexTerrain::Rough);
        assert_eq!(m.len(), 2);
        assert_eq!(m.terrain(HexCoord::new(0, 0)), Some(HexTerrain::Clear));
        assert_eq!(m.terrain(HexCoord::new(50, 50)), None);
        // (1,0) is a neighbor of (0,0); it's the only neighbor present on the map.
        assert!(HexCoord::new(0, 0).is_adjacent(HexCoord::new(1, 0)));
        let ns: Vec<_> = m.neighbors(HexCoord::new(0, 0)).collect();
        assert_eq!(ns, vec![HexCoord::new(1, 0)]);
    }

    #[test]
    fn load_section_with_hexsides() {
        let ron = r#"(
          section: "E",
          hexes: {
            "0101": (terrain: Clear, features: []),
            "0102": (terrain: Clear, features: []),
          },
          hexsides: [
            (a: "0101", b: "0102", kind: Road),
            (a: "0101", b: "0102", kind: Escarpment, up: Some("0101")),
            (a: "0102", b: "D0102", kind: Railroad),
          ],
        )"#;
        let mut m = Map::new();
        m.load_section(ron).unwrap();
        let a = HexId::parse("E0101").unwrap().to_hex_coord();
        let b = HexId::parse("E0102").unwrap().to_hex_coord();
        // Both features coexist on the one edge (§8.37): a Road and an Escarpment (up = a).
        let sides = m.hexsides_on(a, b);
        assert_eq!(sides.len(), 2);
        assert!(sides.iter().any(|h| h.kind == HexsideKind::Road));
        let esc = sides.iter().find(|h| h.kind == HexsideKind::Escarpment).unwrap();
        assert_eq!(esc.high_side, Some(a));
        // A border edge resolves the neighbor via its explicit section letter (D).
        let d = HexId::parse("D0102").unwrap().to_hex_coord();
        assert_eq!(m.hexsides_on(b, d)[0].kind, HexsideKind::Railroad);
    }

    #[test]
    fn load_full_map_all_sections() {
        // All five sections load into one Map without collisions (sum of hexes == map size),
        // confirming the section-offset column scheme.
        let mut m = Map::new();
        let mut total = 0;
        for ron in [
            include_str!("../../../../data/maps/section-a.ron"),
            include_str!("../../../../data/maps/section-b.ron"),
            include_str!("../../../../data/maps/section-c.ron"),
            include_str!("../../../../data/maps/section-d.ron"),
            include_str!("../../../../data/maps/section-e.ron"),
        ] {
            total += m.load_section(ron).unwrap();
        }
        assert_eq!(total, 7117);
        // 91 hexes are shared seam-overlap columns (right sheet's copy wins), so the merged map
        // has fewer unique hexes than the raw section total.
        assert_eq!(m.len(), 7026);
    }

    #[test]
    fn load_section_c_map() {        let ron = include_str!("../../../../data/maps/section-c.ron");
        let mut m = Map::new();
        let n = m.load_section(ron).unwrap();
        // 2170 total - 628 Invalid = 1542 real hexes.
        assert_eq!(n, 1542);
        assert_eq!(m.len(), 1542);

        let hc = |s: &str| HexId::parse(s).unwrap().to_hex_coord();
        // Sollum (C4021): Clear + Village + Port, and coastal.
        let sollum = hc("C4021");
        assert_eq!(m.terrain(sollum), Some(HexTerrain::Clear));
        assert!(m.hex(sollum).unwrap().has_feature(Feature::Village));
        assert!(m.hex(sollum).unwrap().is_port());
        // Sollum (C4021) borders Sea (C4022) -> coastal.
        assert!(m.is_coastal(sollum));
        // Bardia (C4321): Major City with a Port.
        let bardia = hc("C4321");
        assert_eq!(m.terrain(bardia), Some(HexTerrain::MajorCity));
        assert!(m.hex(bardia).unwrap().is_port());
        // An Invalid cell is not loaded.
        assert_eq!(m.terrain(hc("C5101")), None);
        // A deep-desert hex (C0101 = Desert) is not coastal.
        assert!(!m.is_coastal(hc("C0101")));
    }

    #[test]
    fn load_section_a_map() {
        let ron = include_str!("../../../../data/maps/section-a.ron");
        let mut m = Map::new();
        let n = m.load_section(ron).unwrap();
        // 2108 total - 1082 Invalid (charts / Malta box / off-map) = 1026 real hexes.
        assert_eq!(n, 1026);
        assert_eq!(m.len(), 1026);

        let hc = |s: &str| HexId::parse(s).unwrap().to_hex_coord();
        // A0101: SandGravel at the western desert edge.
        assert_eq!(m.terrain(hc("A0101")), Some(HexTerrain::SandGravel));
        // Benghazi (A4827): Major City with a Port.
        let benghazi = hc("A4827");
        assert_eq!(m.terrain(benghazi), Some(HexTerrain::MajorCity));
        assert!(m.hex(benghazi).unwrap().is_port());
        // A2010: a coastal Clear port + village.
        let port = hc("A2010");
        assert!(m.hex(port).unwrap().is_port());
        assert!(m.hex(port).unwrap().has_feature(Feature::Village));
        // An Invalid cell (chart / off-map area) is dropped on load.
        assert_eq!(m.terrain(hc("A0234")), None);
    }

    #[test]
    fn load_section_b_map() {
        let ron = include_str!("../../../../data/maps/section-b.ron");
        let mut m = Map::new();
        let n = m.load_section(ron).unwrap();
        // 2139 total - 145 Invalid = 1994 real hexes.
        assert_eq!(n, 1994);
        assert_eq!(m.len(), 1994);

        let hc = |s: &str| HexId::parse(s).unwrap().to_hex_coord();
        // B0101: Desert at the Libyan Sand Desert edge.
        assert_eq!(m.terrain(hc("B0101")), Some(HexTerrain::Desert));
        // B5230: the section's lone Swamp hex.
        assert_eq!(m.terrain(hc("B5230")), Some(HexTerrain::Swamp));
        // B5925: coastal Rough with a Village + Port.
        let port = hc("B5925");
        assert!(m.hex(port).unwrap().is_port());
        assert!(m.hex(port).unwrap().has_feature(Feature::Village));
        // An Invalid cell is dropped on load.
        assert_eq!(m.terrain(hc("B0135")), None);
    }

    #[test]
    fn load_section_d_map() {
        let ron = include_str!("../../../../data/maps/section-d.ron");
        let mut m = Map::new();
        let n = m.load_section(ron).unwrap();
        // 2170 total - 913 Invalid (Game-Turn Record Track chart) = 1257 real hexes.
        assert_eq!(n, 1257);
        assert_eq!(m.len(), 1257);

        let hc = |s: &str| HexId::parse(s).unwrap().to_hex_coord();
        // D0101: Clear.
        assert_eq!(m.terrain(hc("D0101")), Some(HexTerrain::Clear));
        // D0215: SaltMarsh (the Qattara Depression).
        assert_eq!(m.terrain(hc("D0215")), Some(HexTerrain::SaltMarsh));
        // D3714 (Mersa Matruh): coastal Clear with a Village + Port (matches §60.41).
        let matruh = hc("D3714");
        assert!(m.hex(matruh).unwrap().is_port());
        assert!(m.hex(matruh).unwrap().has_feature(Feature::Village));
        // An Invalid cell (record-track chart) is dropped on load.
        assert_eq!(m.terrain(hc("D0135")), None);
    }

    #[test]
    fn load_section_e_map() {
        let ron = include_str!("../../../../data/maps/section-e.ron");
        let mut m = Map::new();
        let n = m.load_section(ron).unwrap();
        // 2170 total - 872 Invalid (Commonwealth record-track chart) = 1298 real hexes.
        assert_eq!(n, 1298);
        assert_eq!(m.len(), 1298);

        let hc = |s: &str| HexId::parse(s).unwrap().to_hex_coord();
        // E0126: Delta (the Nile Delta).
        assert_eq!(m.terrain(hc("E0126")), Some(HexTerrain::Delta));
        // E3613 (Alexandria): Major City with a Port (matches §60.41).
        let alex = hc("E3613");
        assert_eq!(m.terrain(alex), Some(HexTerrain::MajorCity));
        assert!(m.hex(alex).unwrap().is_port());
        // E1430 (Cairo/Helwan): Major City (matches §60.41).
        assert_eq!(m.terrain(hc("E1430")), Some(HexTerrain::MajorCity));
        // An Invalid cell is dropped on load.
        assert_eq!(m.terrain(hc("E0135")), None);
        // Hexside data is present: E0127|E0227 carries both a Road and the delta Railroad.
        let kinds: Vec<_> = m.hexsides_on(hc("E0127"), hc("E0227")).iter().map(|h| h.kind).collect();
        assert!(kinds.contains(&HexsideKind::Road) && kinds.contains(&HexsideKind::Railroad));
    }

    #[test]
    fn hex_features() {
        use super::Feature::*;
        assert!(Oasis.is_water_source() && Village.is_water_source() && Bir.is_water_source());
        assert!(!Port.is_water_source());

        let mut m = Map::new();
        let sollum = HexCoord::new(201, 1);
        m.set_hex_with_features(sollum, HexTerrain::Clear, vec![Village, Port]);
        let h = m.hex(sollum).unwrap();
        assert_eq!(h.terrain, HexTerrain::Clear);
        assert!(h.has_feature(Village) && h.is_port() && h.has_water_source());
        assert_eq!(m.features(sollum), &[Village, Port]);
        // A plain terrain hex has no features.
        m.set_hex(HexCoord::new(202, 1), HexTerrain::Sea);
        assert!(m.features(HexCoord::new(202, 1)).is_empty());
    }

    #[test]
    fn hexid_parse_and_format() {
        let h = HexId::parse("A2802").unwrap();
        assert_eq!((h.section, h.rr, h.cc), ('A', 28, 2));
        assert_eq!(h.to_string(), "A2802");
        for s in ["A0101", "B5810", "E2555", "C0716"] {
            assert_eq!(HexId::parse(s).unwrap().to_string(), s);
        }
        assert_eq!(HexId::parse("d3328").unwrap().to_string(), "D3328");
    }

    #[test]
    fn hexid_rejects_malformed() {
        for bad in ["", "A", "A123", "A12345", "F0102", "A28X2", "2802"] {
            assert!(HexId::parse(bad).is_none(), "should reject {bad:?}");
        }
    }

    #[test]
    fn hexid_to_hex_coord_and_adjacency() {
        fn hc(s: &str) -> HexCoord {
            HexId::parse(s).unwrap().to_hex_coord()
        }
        // Pointy-top, one continuous grid: HexCoord = (col = section*33 + CC, row = RR).
        // Section C is index 2 -> col +66, no row offset.
        assert_eq!(hc("C0101"), HexCoord::new(67, 1));
        assert_eq!(hc("C0102"), HexCoord::new(68, 1));
        assert_eq!(hc("C0201"), HexCoord::new(67, 2));
        assert_eq!(hc("C4021"), HexCoord::new(87, 40));
        assert_eq!(hc("C4022"), HexCoord::new(88, 40));
        // Sections don't collide.
        assert_ne!(hc("A0101"), hc("B0101"));
        // Adjacency (pointy-top): same row +1 column = E neighbor; the half-row diagonal;
        // Sollum (4021) borders Sea (4022) on its right edge.
        assert!(hc("C0101").is_adjacent(hc("C0102"))); // E
        assert!(hc("C0101").is_adjacent(hc("C0201"))); // diagonal (next row)
        assert!(!hc("C0101").is_adjacent(hc("C0301"))); // two rows away
        assert!(hc("C4021").is_adjacent(hc("C4022"))); // Sollum – Sea
    }
}
