//! Two experienced players' openings from Quicksilver's north start (their replays run through
//! `run/replay_match.py`, 2026-09-20; K-open-early-pawn-pressure-is-standard) are inside the search space: every step is
//! on the palette, and the simulator plays each queue through in order without passing a step over. The search need
//! not choose them; it must be able to.

use buildorder::anneal::{Palette, Search, Objective};
use buildorder::game::Game;
use buildorder::plan::{Item, Plan, Step};
use buildorder::sim::{simulate, State};

fn game() -> Game {
    buildorder::record::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/quicksilver-nw.jsonl"), 60.0).unwrap().game
}

fn queue(game: &Game, names: &str) -> Vec<Step> {
    names.split_whitespace().map(|n| if n == "assist" { Step { item: Item::Assist, site: None } } else { Step::build(game.units.index(n).unwrap_or_else(|| panic!("no {n}"))) }).collect()
}

/// (commander, lab, constructors), as the engine recorded each builder's units in creation order, within five minutes;
/// `assist` where the replay's command stream has the commander guarding the lab.
fn openings(game: &Game) -> Vec<(&'static str, Plan)> {
    let mut player2 = Plan::empty(1, 2);
    player2.commander = queue(game, "armmex armmex armwin armwin armwin armlab armwin armsolar armwin armwin armwin armwin armwin assist assist armmakr armwin armwin armwin");
    player2.factories[0] = queue(game, "armck armpw armpw armpw armpw armck armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw");
    player2.constructors[0] = queue(game, "armmex armmex armllt armrad armmex armllt armmex");
    player2.constructors[1] = queue(game, "armmex armllt armmex armllt");
    let mut ben = Plan::empty(1, 2);
    ben.commander = queue(game, "armmex armmex armsolar armsolar armlab armsolar armsolar assist assist assist assist armsolar armsolar armsolar");
    ben.factories[0] = queue(game, "armck armck armpw armpw armpw armpw armpw armpw armpw armck armpw armpw armpw armpw armrectr armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armpw armrectr armpw");
    ben.constructors[0] = queue(game, "armmex armrad armllt armmex armmex armmex");
    ben.constructors[1] = queue(game, "armmex armmex armmex armmex");
    vec![("player 2", player2), ("Ben", ben)]
}

#[test]
fn both_players_openings_are_on_the_palette() {
    let game = game();
    let units = &game.units;
    let palette = Palette::new(units, game.commander, game.factory("lab").unwrap(), true, units.index("armllt"));
    for (who, plan) in openings(&game) {
        for (q, steps) in [vec![plan.commander.clone()], plan.factories.clone(), plan.constructors.clone()].concat().iter().enumerate() {
            let offered = if q == 1 { &palette.factory } else { &palette.mobile };
            for step in steps {
                let name = match step.item { Item::Build(u) => units.list[u].name.clone(), Item::Assist => "assist".into() };
                assert!(offered.contains(&step.item), "{who}: {name} in queue {q} is not on the palette");
            }
        }
    }
}

#[test]
fn both_players_openings_simulate_without_a_step_passed_over() {
    let game = game();
    let mut scenario = game.scenario(game.own_half(), game.ground());
    // The 3.5 s a build costs our executor between orders (`mobile_overhead`, calibrated on our own games) is ours, not
    // a human's: a player queues orders and loses nothing.
    scenario.mobile_overhead = 0.0;
    for (who, plan) in openings(&game) {
        let outcome = simulate(&game.units, &scenario, &State::start(&scenario), &plan, 300.0);
        for q in 0..plan.queue_count() {
            let planned = plan.queue(q);
            let taken = &outcome.effective[q];
            assert!(taken.len() <= planned.len(), "{who}: queue {q} took more steps than planned");
            for (i, step) in taken.iter().enumerate() {
                assert_eq!(step.item, planned[i].item, "{who}: queue {q} passed a step over at {i}");
            }
        }
        let last = outcome.last();
        let lab = outcome.finished.iter().find(|f| game.units.list[f.unit].name == "armlab").map(|f| f.t);
        let soldiers = |by: f64| outcome.finished.iter().filter(|f| f.t <= by && game.units.list[f.unit].name == "armpw").count();
        eprintln!("{who}: lab at {:?} s; Pawns by 2 / 3 / 4 min: {} / {} / {}; extractors {} income {:.1} at 5 min", lab, soldiers(120.0), soldiers(180.0), soldiers(240.0), last.extractors, last.metal_income);
        assert!(lab.is_some_and(|t| t < 120.0), "{who}: the lab never came up in time");
        assert!(soldiers(180.0) >= 5, "{who}: fewer than five Pawns by minute 3");
        let _ = Search { objective: Objective::Income, horizon: 300.0, iterations: 0, seed: 0, factories: 1, constructors: 2, hot: 0.0, start: None };
    }
}
