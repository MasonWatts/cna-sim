# Integrated Sequence of Play (§33.0 Land/Air Game-Turn)

The engine's `advance_phase` now walks the full integrated Game-Turn (Stages I–VIII):

```
I.   INITIATIVE DETERMINATION STAGE            (once/GT)  Phase::InitiativeDetermination
II.  STRATEGIC AIR PLANNING STAGE   (once/GT, PRE_TURN_SEQUENCE)
       A Designation                 StrategicAirDesignation      (Initiative)
       B Axis Malta Availability      MaltaAvailabilityDetermination (Axis)
       C Strategic Mission Assignment StrategicMissionAssignment   (Initiative)
       D Malta Raid                   MaltaRaid                    (Axis)
III. NAVAL CONVOY STAGE             (once/GT, PRE_TURN_SEQUENCE)
       A Naval Convoy Schedule        NavalConvoySchedule          (Axis)
       B Convoy Resolution            ConvoyResolution             (Initiative)
IV/V/VI. OPERATIONS STAGES 1-3      (each, STAGE_SEQUENCE)
       A Initiative Declaration       (declare_initiative_order)
       B Weather                      Weather                      (Initiative)
       D Naval Convoy Arrival         NavalConvoyArrival           (Initiative)  [reinforcements]
       C Organization                 Organization                (Initiative)
       E Commonwealth Fleet           CommonwealthFleet            (Commonwealth)
       F Land Support Air             LandSupportAir               (Initiative)
       G-M per player (A then B):     ReserveDesignation, MovementAndCombat,
                                      TruckConvoy, CommonwealthRail, Repair, Patrol
VII. STRATEGIC AIR RECOVERY STAGE   (once/GT, POST_TURN_SEQUENCE)
       A Return to Base               StrategicAirReturn           (Initiative)
       B Aircraft Maintenance         AircraftMaintenance          (Initiative)
VIII.END OF GAME-TURN               -> next GT, re-determine Initiative
```

Notes:
- `pre_turn_step` / `post_turn_step` track Stages II–III / VII; `stage_step` walks the
  Operations Stage. `operations_stage` is 0 during the pre-turn stages.
- StageActor gained Axis / Commonwealth for the side-specific air/naval phases.
- The Organization/NavalConvoyArrival order preserves the existing land behaviour (arrivals
  land at Naval Convoy Arrival, §20.12, then can be organized).
- PLAYER ACTIONS: actions across every stage are exposed as phase-gated `Command`s through
  `apply` (wrong-phase → `CommandError::WrongPhase`, tested end-to-end through the turn sequence).
  Naval/convoy: `PlanConvoy` (Naval Convoy Schedule), `BombConvoy`/`ReconConvoyLane` (Convoy
  Resolution), `DeliverConvoy` (Naval Convoy Arrival), `BlockHarbor`/`UnblockHarbor` (§55.2).
  Commonwealth Fleet Phase: `AssignShip`/`RepairShip` (§30) [+ `OffShoreBombard` fires in the
  Movement & Combat Barrage Step]. Malta: `DetermineMaltaAvailability` (Malta Availability),
  `BombMaltaBase` (Malta Raid, §44.24), `ConstructMaltaFacility` (Organization, §44.13). Reserve
  Designation: `DesignateReserve` (§18). Repair Phase: `Repair` (§22 truck/tank/recce/facility).
  Aircraft Maintenance (Stage VII.B) shares refit/ready/arm with Land Support Air. Plus the large
  land/logistics/air command set (movement, combat, construction, rail, replacements, squadron
  ready/arm/fly, bombing, etc.).
- COVERAGE: **every Sequence-of-Play phase that has backing state now has player-action commands.**
  The remaining gaps require *new subsystems* rather than wiring: Strategic Air Designation /
  Strategic Mission Assignment (Stage II.A/C — no per-squadron mission-designation state), Patrol
  (IV.M), and unattached-truck / POW movement in Truck Convoy (IV.J; combat units move via
  `MoveStep`). The Malta Raid gap (Stage II.D) is now closed. See `commonwealth-fleet.md`,
  `scenario-setup.md`, and `air-game.md` for per-subsystem deferrals.
