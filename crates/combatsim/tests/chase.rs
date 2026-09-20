//! What a chase question must keep answering (`docs/design/2026-09-20-army-response.md`, part 1).

use combatsim::chase::Chase;
use combatsim::sim::Rules;
use combatsim::{Intent, Vec2};

fn outpost(rules: &Rules, pursuers: (&str, u32), distance: f32, intent: Intent) -> Chase {
    let unit = |name: &str| rules.units.index(name).expect(name);
    Chase {
        pursuers: if pursuers.1 > 0 { vec![(unit(pursuers.0), pursuers.1, Vec2::new(-distance, 0.0))] } else { Vec::new() },
        party: vec![(unit("corak"), 3)],
        at: Vec2::new(0.0, 0.0),
        intent,
        assets: (0..4).map(|i| (unit("armmex"), Vec2::new(150.0, 250.0 * (i as f32 - 1.5)))).collect(),
        seconds: 60.0,
    }
}

const RAID: Intent = Intent::Raid { then: Vec2 { x: 4000.0, z: 0.0 } };

#[test]
fn raiders_left_alone_burn_the_outpost_and_leave() {
    let rules = Rules::default();
    let verdict = outpost(&rules, ("armham", 0), 1500.0, RAID).verdict(&rules, 4);
    assert_eq!(verdict.assets_lost, 200.0, "four extractors at 50 metal");
    assert_eq!((verdict.party_killed, verdict.survived), (0.0, 1.0));
}

#[test]
fn pursuers_from_far_away_arrive_to_ashes_and_are_worth_nothing() {
    let rules = Rules::default();
    let nobody = outpost(&rules, ("armham", 0), 1500.0, RAID).verdict(&rules, 4);
    for pursuer in ["armham", "armpw"] {
        let sent = outpost(&rules, (pursuer, 4), 1500.0, RAID).verdict(&rules, 4);
        assert_eq!(sent.caught, 0.0, "{pursuer} from 1500 away");
        assert_eq!(sent.gain_over(&nobody), 0.0);
    }
}

#[test]
fn pursuers_close_by_catch_raiders_at_their_work() {
    let rules = Rules::default();
    let nobody = outpost(&rules, ("armham", 0), 600.0, RAID).verdict(&rules, 4);
    let maces = outpost(&rules, ("armham", 4), 600.0, RAID).verdict(&rules, 4);
    let pawns = outpost(&rules, ("armpw", 4), 600.0, RAID).verdict(&rules, 4);
    assert!(maces.caught == 1.0 && pawns.caught == 1.0);
    assert!(pawns.caught_after < maces.caught_after, "the faster unit lands the first hit sooner");
    assert!(maces.gain_over(&nobody) > 100.0 && pawns.gain_over(&nobody) > 100.0);
}

#[test]
fn a_slow_unit_never_catches_a_faster_one_that_runs() {
    let rules = Rules::default();
    let verdict = outpost(&rules, ("armham", 4), 600.0, Intent::Flee(Vec2::new(4000.0, 0.0))).verdict(&rules, 4);
    assert_eq!((verdict.caught, verdict.party_killed, verdict.survived), (0.0, 0.0, 1.0));
}

#[test]
fn a_guard_stands_until_something_comes_within_its_radius() {
    let rules = Rules::default();
    let unit = |name: &str| rules.units.index(name).expect(name);
    // Guards at the outpost already: the raiders walk into them.
    let mut chase = outpost(&rules, ("armham", 4), 100.0, RAID);
    chase.party = vec![(unit("corak"), 3)];
    chase.at = Vec2::new(1200.0, 0.0);
    let mut scenario = chase.scenario();
    for group in scenario.sides[0].iter_mut().filter(|g| rules.units.list[g.def].mobile()) {
        group.intent = Intent::Guard { at: Vec2::new(0.0, 0.0), radius: 500.0 };
    }
    let outcome = combatsim::simulate(&rules, &scenario, 0);
    assert!(outcome.metal_lost[1] > 0.0, "the raiders met the guard");
    assert!(outcome.assets_lost[0] < 200.0, "and did not burn everything");
}
