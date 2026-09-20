//! What the model must keep doing. The regression at the bottom pins the agreement with the engine duel tables;
//! the rest pins the mechanisms that agreement rests on, so a change that trades one for another is visible.

use combatsim::duels::{self, DECISIVE};
use combatsim::field::Field;
use combatsim::scenario::{End, Energy};
use combatsim::sim::{Rules, Tuning};
use combatsim::units::{Units, aim_error};
use combatsim::{Group, Scenario, Vec2, odds, simulate};

fn rules() -> Rules {
    Rules::default()
}

fn blob(rules: &Rules, name: &str, count: u32, x: f32, facing: f32) -> Group {
    let mut group = Group::new(rules.units.index(name).expect(name), count, Vec2::new(x, 0.0), Vec2::new(facing, 0.0));
    group.spacing = 56.0;
    group
}

fn duel(rules: &Rules, a: (&str, u32), b: (&str, u32)) -> Scenario {
    let mut scenario = Scenario::new();
    scenario.sides[0] = vec![blob(rules, a.0, a.1, 0.0, 1.0)];
    scenario.sides[1] = vec![blob(rules, b.0, b.1, duels::APART, -1.0)];
    scenario
}

#[test]
fn unit_numbers_are_the_games_own() {
    let units = Units::default();
    let mace = units.get("armham");
    assert_eq!((mace.metal, mace.health, mace.speed), (130.0, 1000.0, 46.2));
    assert_eq!(mace.weapons[0].range, 380.0);
    // Reload is rounded down to whole engine frames, as `alldefs_post` does: 1.73333 -> 52 frames.
    assert_eq!((mace.weapons[0].reload * 30.0).round(), 52.0);
    // Janus mounts the same launcher twice and fires both at once; one weapondef, two weapons.
    assert_eq!(units.get("armjanus").weapons.len(), 2);
    // Anti-air does not exist as far as a land fight is concerned.
    assert_eq!(units.get("armjeth").reach(), 0.0);
}

#[test]
fn the_advanced_labs_line_is_in_the_table() {
    let units = Units::default();
    for name in [
        "armzeus", "armmav", "armfido", "armsnipe", "armfboy", "armsptk", "armfast", "armamph", "corcan",
        "corsumo", "corpyro", "corhrk", "cormort", "cortermite", "coramph",
    ] {
        assert!(units.get(name).reach() > 0.0, "{name} should have a land weapon");
    }
    // The labs' constructors and support bots come along too, and are unarmed.
    for name in ["armack", "corack", "corfast", "armfark"] {
        assert!(units.get(name).builder && units.get(name).reach() == 0.0, "{name} builds and does not shoot");
    }
    // The ones whose whole mechanism the model has no answer for are left out, not quietly left harmless.
    for name in ["armvader", "corroach", "corsktl", "armspid"] {
        assert!(units.index(name).is_none(), "{name} should be excluded from the table");
    }
}

#[test]
fn weapons_that_take_no_part_in_a_land_fight_are_dropped() {
    let units = Units::default();
    // A paralyser is not damage (Shockwave), a stockpiled launcher has nothing built (nuclear silo), a torpedo
    // needs water (Duck keeps its laser and loses its torpedo), and the D-Gun is fired by hand, not by itself.
    assert_eq!(units.get("armshockwave").reach(), 0.0);
    assert_eq!(units.get("armsilo").reach(), 0.0);
    assert_eq!(units.get("coramph").reach(), 300.0);
    assert_eq!(units.get("armcom").reach(), 300.0);
    assert_eq!(units.get("armcom").weapons.iter().filter(|w| w.hits_ground()).count(), 1);
    // A smart-trajectory plasma battery mounts its gun twice and fires it once (see the extraction tool).
    for name in ["armguard", "corpun", "armamb", "cortoast"] {
        let firing = units.get(name).weapons.iter().filter(|w| w.hits_ground()).count();
        assert_eq!(firing, 1, "{name} should fire one plasma shot a reload, not two");
    }
}

#[test]
fn aim_error_uses_the_engines_conversion() {
    // WeaponDef.cpp's AccuracyToSin: sin(x * pi / 0xafff). The Pawn's spray of 1180 is about 4.7 degrees.
    assert!((aim_error(1180.0) - 0.0822).abs() < 1e-3);
    assert_eq!(aim_error(0.0), 0.0);
}

#[test]
fn a_faster_gun_kills_sooner() {
    let rules = rules();
    let quick = simulate(&rules, &duel(&rules, ("armpw", 10), ("armlab", 1)), 0);
    let slow = simulate(&rules, &duel(&rules, ("armrock", 10), ("armlab", 1)), 0);
    assert_eq!(quick.winner, Some(0));
    assert!(quick.seconds < slow.seconds, "{} against {}", quick.seconds, slow.seconds);
}

#[test]
fn the_same_seed_replays_exactly() {
    let rules = rules();
    let scenario = duel(&rules, ("armham", 9), ("corak", 28));
    let a = simulate(&rules, &scenario, 7);
    let b = simulate(&rules, &scenario, 7);
    assert_eq!(a.margin, b.margin);
    assert_eq!(a.seconds, b.seconds);
    assert_eq!(a.survivors[0], b.survivors[0]);
    // And different seeds are actually different fights.
    let spread: Vec<f32> = (0..8).map(|s| simulate(&rules, &scenario, s).margin).collect();
    assert!(spread.iter().any(|m| *m != spread[0]), "every seed gave {}", spread[0]);
}

#[test]
fn energy_is_what_holds_laser_towers_back() {
    // The Sentry has the best damage per metal in the tier-1 table and still loses to Maces, because each shot
    // costs 20 energy and a duel team makes 30 a second (K-units-laser-towers-need-energy).
    let rules = rules();
    let mut scenario = duel(&rules, ("armham", 9), ("armllt", 14));
    let starved = odds(&rules, &scenario, 4).mean_margin;
    scenario.energy[1] = Energy { stored: 100_000.0, income: 10_000.0 };
    let fed = odds(&rules, &scenario, 4).mean_margin;
    assert!(starved > 0.2, "Maces should beat starved towers, got {starved}");
    assert!(fed < starved - 0.5, "a fed tower line should do far better: {fed} against {starved}");
}

#[test]
fn units_arriving_late_lose_a_fight_their_value_should_win() {
    let rules = rules();
    let together = duel(&rules, ("armham", 12), ("corthud", 11));
    let mut strung_out = together.clone();
    let half = strung_out.sides[0][0].count / 2;
    strung_out.sides[0][0].count -= half;
    let mut late = strung_out.sides[0][0].clone();
    late.count = half;
    late.delay = 10.0;
    strung_out.sides[0].push(late);
    let (a, b) = (odds(&rules, &together, 6).mean_margin, odds(&rules, &strung_out, 6).mean_margin);
    // The effect is real but small at this size: a wave that arrives ten seconds late is worth about five
    // points of margin. It is much larger when the late half never joins at all.
    assert!(b < a - 0.05, "arriving in two halves should cost: {b} against {a}");
}

#[test]
fn collision_saturates_a_short_range_blob() {
    // Sixty-four Pawns cannot all stand within 180 elmos of one building; long-ranged Rocketeers can.
    let scale = |name: &str, collide: bool| {
        let rules = Rules::new(Units::default(), Tuning { collide, ..Tuning::default() });
        let damage = |n| {
            let out = simulate(&rules, &duel(&rules, (name, n), ("armlab", 8)), 0);
            let shooting = out.seconds - out.contact_seconds.expect("contact");
            23_200.0 / shooting.max(0.05)
        };
        damage(64) / damage(8)
    };
    let (crowded, free) = (scale("armpw", true), scale("armpw", false));
    assert!(crowded < free * 0.8, "crowding should cost a close-range blob: {crowded} against {free}");
    assert!(scale("armrock", true) > scale("armrock", false) * 0.8, "range should keep scaling");
}

/// A wall of cliff across the middle, `thick` cells deep, with a gap `gap_cells` wide in it.
fn choke(gap_cells: usize, thick: usize) -> Field {
    let (width, height) = (100, 60);
    let mut field = Field::flat(width, height, 16.0);
    let band = (height / 2 - thick / 2)..(height / 2 + thick.div_ceil(2));
    for z in band.clone() {
        for x in 0..width {
            let gap = x >= width / 2 - gap_cells / 2 && x < width / 2 + gap_cells.div_ceil(2);
            field.slopes[z * width + x] = if gap { 0 } else { 255 };
        }
    }
    field
}

#[test]
fn a_choke_costs_the_side_that_has_to_come_through_it() {
    let rules = rules();
    let attack = |terrain: Option<Field>| {
        let mut scenario = Scenario::new();
        let mut a = Group::new(rules.units.index("armpw").unwrap(), 24, Vec2::new(800.0, 200.0), Vec2::new(0.0, 1.0));
        let mut b = Group::new(rules.units.index("armham").unwrap(), 10, Vec2::new(800.0, 700.0), Vec2::new(0.0, -1.0));
        (a.spacing, b.spacing) = (56.0, 56.0);
        scenario.sides = [vec![a], vec![b]];
        scenario.terrain = terrain;
        odds(&rules, &scenario, 4).mean_margin
    };
    let open = attack(Some(Field::flat(100, 60, 16.0)));
    let narrow = attack(Some(choke(3, 3)));
    assert!(narrow < open - 0.1, "coming through a 48-elmo gap should cost: {narrow} against {open}");
}

#[test]
fn terrain_can_cut_a_fight_off_entirely() {
    let rules = rules();
    let mut scenario = Scenario::new();
    let mut a = Group::new(rules.units.index("armpw").unwrap(), 6, Vec2::new(800.0, 200.0), Vec2::new(0.0, 1.0));
    let mut b = Group::new(rules.units.index("armpw").unwrap(), 6, Vec2::new(800.0, 700.0), Vec2::new(0.0, -1.0));
    (a.spacing, b.spacing) = (56.0, 56.0);
    scenario.sides = [vec![a], vec![b]];
    // Thick enough that nothing can shoot across it either.
    scenario.terrain = Some(choke(0, 30));
    scenario.stalemate = 20.0;
    let outcome = simulate(&rules, &scenario, 0);
    assert_eq!(outcome.reason, End::Stalemate);
    assert_eq!(outcome.margin, 0.0);
}

/// The headline number the study reports. Raise it when the model improves; never lower it quietly.
#[test]
fn agreement_with_the_engine_duel_tables() {
    let rules = rules();
    for (pairs, spacing, floor, error) in
        [(duels::tight(), 56.0, 0.75, 0.31), (duels::wide(), 100.0, 0.80, 0.26)]
    {
        let got = duels::validate(&rules, &pairs, spacing, 2, Energy::default());
        let share = got.same_sign as f32 / got.decisive as f32;
        assert!(got.decisive > 450, "spacing {spacing}: only {} decisive pairings", got.decisive);
        assert!(share >= floor, "spacing {spacing}: sign agreement {share:.3} below {floor} (|margin| >= {DECISIVE})");
        assert!(got.mean_absolute_error <= error, "spacing {spacing}: mean error {:.3}", got.mean_absolute_error);
    }
}
