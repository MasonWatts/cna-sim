//! Diagnostic: SGSU drive costs to the §36.53 entry hexes, and plane ranges vs real air distances
//! from the Commonwealth off-map fields. Usage: air_costs
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use cna_engine::reference::terrain::MoveCost;
use cna_engine::rules::movement::{step_cost, MoveMode};
use cna_engine::state::map::{HexCoord, HexId};

fn hx(s: &str) -> HexCoord { HexId::parse(s).unwrap().to_hex_coord() }

fn main() {
    let (g, _) = cna_replay::scenarios::build("campaign", 1);
    // Motorized shortest-path CP (tenths) over the real map (Dijkstra on step_cost).
    let drive = |from: HexCoord, to: HexCoord| -> Option<i32> {
        let mut best: HashMap<HexCoord, i32> = HashMap::new();
        let mut pq = BinaryHeap::new();
        best.insert(from, 0);
        pq.push(Reverse((0, from.col, from.row)));
        while let Some(Reverse((d, c, r))) = pq.pop() {
            let h = HexCoord::new(c, r);
            if h == to { return Some(d); }
            if best.get(&h).is_some_and(|&b| b < d) { continue; }
            for n in g.map.neighbors(h) {
                if let Some(MoveCost::Cost(f)) = step_cost(&g.reference, &g.map, h, n, MoveMode::Motorized) {
                    let nd = d + f.0.max(0);
                    if best.get(&n).is_none_or(|&b| nd < b) { best.insert(n, nd); pq.push(Reverse((nd, n.col, n.row))); }
                }
            }
        }
        None
    };
    println!("== SGSU (CW CPA 25) drive cost to each entry hex (motorized CP over the real map)");
    let bases = [("Cairo", "E1830"), ("Helwan", "E1430"), ("Alexandria", "E3613"), ("Amiriya", "E3212"), ("Mersa Matruh", "D3714"), ("Maaten Baggush", "D3520")];
    for entry in ["E1833", "E3433", "E4033", "E0127"] {
        let row: Vec<String> = bases.iter().map(|(n, h)| format!("{n} {}", drive(hx(h), hx(entry)).map_or("—".into(), |t| format!("{:.1}", t as f64 / 10.0)))).collect();
        println!("  to {entry}: {}", row.join(" | "));
    }
    let chart = g.reference.off_map_air_facilities.as_ref().unwrap();
    println!("== Option 2 cost of the field leg (distance as CP) and alternatives");
    for f in &chart.facilities {
        println!("  {:<10} entry {} distance {:>2}: as 1 CP/hex = {:>2} CP | as road (1/2) = {:>4.1} | as clear (2) = {:>3}  (CPA 25)",
            f.name, f.entry_hex, f.distance, f.distance, f.distance as f64 / 2.0, f.distance * 2);
    }
    println!("== Air distance from each off-map field to targets (§36.53 distance + hex distance from entry hex)");
    let targets = [("Alexandria", "E3613"), ("Mersa Matruh", "D3714"), ("Maaten Baggush", "D3520"), ("Sidi Barrani", "C4131"), ("Bardia", "C4321"), ("Tobruk", "C4807"), ("Derna", "B5925"), ("Benghazi", "A4827")];
    let names: Vec<&str> = targets.iter().map(|t| t.0).collect();
    println!("  {:<10} {}", "", names.iter().map(|n| format!("{:>14}", n)).collect::<String>());
    for f in chart.facilities.iter().filter(|f| f.name != "Ethiopia") {
        let row: String = targets.iter().map(|(_, h)| format!("{:>14}", g.off_map_facility_flight_distance(&f.name, hx(h)).unwrap())).collect();
        println!("  {:<10} {row}", f.name);
    }
    println!("== On-map field to field (hex distance)");
    for (a, ah, b, bh) in [("Mersa Matruh","D3714","Sidi Barrani","C4131"), ("Mersa Matruh","D3714","Bardia","C4321"), ("Maaten Baggush","D3520","Tobruk","C4807"), ("Mersa Matruh","D3714","Tobruk","C4807"), ("Alexandria","E3613","Mersa Matruh","D3714"), ("Amiriya","E3212","Sidi Barrani","C4131"), ("Cairo","E1830","Benghazi","A4827"), ("Benghazi","A4827","Alexandria","E3613")] {
        println!("  {a} -> {b}: {}", hx(ah).distance(hx(bh)));
    }
}
