//! The simulator's arithmetic against hand calculations from the unit numbers of a real game: the fixture is the
//! header of a Quicksilver record (2026-09-20) cut down to both factions' tier-1 land units, and its commander.

use std::sync::Arc;

use buildorder::anneal::{anneal, Objective, Palette, Search};
use buildorder::game::{Game, Spot, Straight};
use buildorder::plan::{Item, Plan, Step};
use buildorder::sim::{simulate, Scenario};
use buildorder::units::{Role, Units};

const HOME: (f64, f64) = (1000.0, 1000.0);

fn game() -> Game {
    buildorder::record::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/quicksilver-nw.jsonl"), 60.0).unwrap().game
}

/// No walking overheads and a fine step, so that times can be checked against `buildtime / workertime`.
fn bare(game: &Game) -> Scenario {
    let spots = [(1000.0, 1000.0), (1000.0, 3000.0)].map(|at| Spot { at, metal: 2.0 }).to_vec();
    let mut scenario = Scenario::new(game.commander, HOME, spots, Arc::new(Straight { detour: 1.0 }), 10.0);
    scenario.dt = 0.1;
    scenario.reach_bonus = 0.0;
    scenario.mobile_overhead = 0.0;
    scenario.walk_overhead = 0.0;
    scenario.factory_overhead = 0.0;
    scenario
}

fn at_home(units: &Units, names: &[&str]) -> Vec<Step> {
    names.iter().map(|n| Step { item: Item::Build(units.index(n).unwrap()), site: Some(HOME) }).collect()
}

fn finish_time(units: &Units, outcome: &buildorder::sim::Outcome, name: &str) -> f64 {
    outcome.finished.iter().find(|f| units.list[f.unit].name == name).unwrap_or_else(|| panic!("{name} never finished")).t
}

#[test]
fn the_engine_reports_the_documented_numbers() {
    let game = game();
    let units = &game.units;
    let mex = units.get("armmex");
    assert_eq!((mex.metal_cost, mex.energy_cost, mex.build_time), (50.0, 500.0, 1800.0));
    assert_eq!(mex.energy_make, -3.0, "extractor upkeep");
    assert_eq!(units.get("armsolar").energy_make, 20.0, "solars produce through a negative upkeep");
    assert_eq!(units.get("armcom").worker_time, 300.0);
    assert_eq!(units.get("armnanotc").worker_time, 200.0);
    assert_eq!(units.get("armck").role, Role::Builder);
    assert_eq!(units.get("armlab").role, Role::Factory);
    assert_eq!(units.get("armpw").role, Role::Army);
    assert_eq!(units.get("armllt").role, Role::Turret);
    assert_eq!(units.get("armnanotc").role, Role::Nano);
    assert_eq!(units.list[game.commander].role, Role::Commander);
    assert!((game.spot_metal(game.spots[0].1) - 2.0).abs() < 0.01, "a Quicksilver spot pays 2 metal/s");
}

#[test]
fn build_time_is_buildtime_over_build_power_and_cost_is_paid_once() {
    let game = game();
    let units = &game.units;
    let mut plan = Plan::empty(1, 0);
    plan.commander = at_home(units, &["armwin"]);
    let outcome = simulate(units, &bare(&game), &plan, 20.0);
    // 1600 / 300 = 5.33 s, after the 0.1 s step in which the order is taken up.
    assert!((finish_time(units, &outcome, "armwin") - 5.43).abs() < 0.11);
    let end = outcome.last();
    // Metal: 1000 + 2/s from the commander (capped at 1000 until the turbine starts draining) - 40.
    assert!((end.metal - (1000.0 - 40.0 + 2.0 * 20.0)).abs() < 1.5, "metal {}", end.metal);
    assert!((end.energy_income - 40.0).abs() < 1e-9, "30 from the commander + 10 wind, got {}", end.energy_income);
}

#[test]
fn a_stalled_build_runs_at_the_speed_of_the_scarce_resource() {
    let game = game();
    let units = &game.units;
    let mut scenario = bare(&game);
    scenario.start_energy = 0.0;
    let mut plan = Plan::empty(1, 0);
    plan.commander = at_home(units, &["armmex"]);
    let outcome = simulate(units, &scenario, &plan, 30.0);
    // 500 E at the commander's 30 E/s: 16.7 s instead of 1800 / 300 = 6 s.
    assert!((finish_time(units, &outcome, "armmex") - 16.8).abs() < 0.3);
    let stalled = &outcome.samples[5];
    assert!((stalled.stall - 30.0 / (500.0 / 6.0)).abs() < 0.02, "stall factor {}", stalled.stall);
    assert_eq!(outcome.last().extractors, 1);
    assert!((outcome.last().metal_income - 4.0).abs() < 1e-9, "commander 2 + one spot at 2");
}

#[test]
fn walking_takes_distance_beyond_reach_over_speed() {
    let game = game();
    let units = &game.units;
    let mut plan = Plan::empty(1, 0);
    plan.commander = vec![Step { item: Item::Build(units.index("armmex").unwrap()), site: Some((1000.0, 3000.0)) }];
    let outcome = simulate(units, &bare(&game), &plan, 80.0);
    let expected = (2000.0 - 145.0) / 37.5 + 6.0;
    assert!((finish_time(units, &outcome, "armmex") - expected).abs() < 0.3);
}

#[test]
fn converters_burn_only_energy_above_three_quarters_of_storage() {
    let game = game();
    let units = &game.units;
    let mut plan = Plan::empty(1, 0);
    plan.commander = at_home(units, &["armmakr", "armsolar", "armsolar", "armsolar"]);
    let outcome = simulate(units, &bare(&game), &plan, 200.0);
    let built = finish_time(units, &outcome, "armmakr");
    // Right after the converter finishes, stored energy is below 750 (1150 was just spent): no conversion.
    let early = outcome.samples.iter().find(|s| s.t > built + 1.0).unwrap();
    assert!(early.energy < 750.0 && (early.metal_income - 2.0).abs() < 1e-9);
    // At the end 30 + 3 x 20 = 90 E/s comes in and the converter takes its full 70 E/s for 70 x 0.01429 = 1.0 M/s.
    let end = outcome.last();
    assert!((end.metal_income - 3.0).abs() < 0.01, "metal income {}", end.metal_income);
    assert!(end.energy >= 0.75 * 1250.0 - 1.0, "energy {} held at or above the converter level", end.energy);
}

#[test]
fn construction_turrets_add_their_build_power_to_the_factory() {
    let game = game();
    let units = &game.units;
    let time_of_second_pawn = |with_nano: bool| {
        let mut scenario = bare(&game);
        (scenario.start_metal, scenario.start_energy, scenario.base_storage) = (20_000.0, 20_000.0, 20_000.0);
        // The commander has no construction turret in its menu; a constructor does.
        let mut plan = Plan::empty(1, 1);
        plan.commander = at_home(units, &["armlab"]);
        plan.factories[0] = at_home(units, &[&["armck"][..], &["armpw"; 20][..]].concat());
        plan.constructors[0] = at_home(units, if with_nano { &["armnanotc"] } else { &[] });
        let outcome = simulate(units, &scenario, &plan, 400.0);
        let pawns: Vec<f64> = outcome.finished.iter().filter(|f| units.list[f.unit].name == "armpw").map(|f| f.t).collect();
        pawns[19] - pawns[18]
    };
    assert!((time_of_second_pawn(false) - 1650.0 / 150.0).abs() < 0.11);
    assert!((time_of_second_pawn(true) - 1650.0 / 350.0).abs() < 0.11);
}

#[test]
fn plan_text_round_trips() {
    let game = game();
    let units = &game.units;
    let text = "com: mex win lab@1952,1472 assist\nfac0: ck pw\ncon0: mex nanotc\n";
    let plan = Plan::from_text(text, "arm", units).unwrap();
    assert_eq!(plan.to_text(units), text);
    assert!(Plan::from_text("com: nosuchunit", "arm", units).is_err());
}

#[test]
fn annealing_is_deterministic_and_beats_its_seed_plan() {
    let game = game();
    let units = &game.units;
    let mut scenario = game.scenario(game.own_half(), game.ground());
    scenario.constructors_default_to_extractors = true;
    let palette = Palette::new(units, game.commander, game.factory("lab").unwrap(), true);
    let search = Search { objective: Objective::Mix, horizon: 300.0, iterations: 1500, seed: 7, factories: 2, constructors: 6, hot: 0.02 };
    let seed_outcome = simulate(units, &scenario, &palette.seed_plan(2, 6), 300.0);
    let (a, b) = (anneal(units, &scenario, &palette, &search), anneal(units, &scenario, &palette, &search));
    assert_eq!(a.plan, b.plan);
    assert_eq!(a.score, b.score);
    assert!(a.score > Objective::Mix.score(&seed_outcome, 300.0));
    // The plan handed back reproduces its score when simulated afresh.
    let again = simulate(units, &scenario, &a.plan, 300.0);
    assert!((Objective::Mix.score(&again, 300.0) - a.score).abs() < 1e-9);
}
