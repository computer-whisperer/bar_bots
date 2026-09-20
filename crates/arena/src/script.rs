//! Start script generation (`doc/StartScriptFormat.txt` in the engine repo).

/// How the ally teams' start boxes lie on the map. The first box is where "first" plays (`we_are_first`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Boxes {
    /// North-west against south-east 30 % corners (then north-east, south-west for a third and fourth ally team).
    Corners,
    /// The northern quarter against the southern one.
    NorthSouth,
    /// The western quarter against the eastern one.
    WestEast,
}

impl Boxes {
    /// `(left, top, right, bottom)` as fractions of the map, and the box's name in results.
    fn rect(self, index: usize) -> ([f32; 4], &'static str) {
        match (self, index) {
            (Boxes::Corners, 0) => ([0.0, 0.0, 0.3, 0.3], "NW"),
            (Boxes::Corners, 1) => ([0.7, 0.7, 1.0, 1.0], "SE"),
            (Boxes::Corners, 2) => ([0.7, 0.0, 1.0, 0.3], "NE"),
            (Boxes::Corners, _) => ([0.0, 0.7, 0.3, 1.0], "SW"),
            (Boxes::NorthSouth, 0) => ([0.0, 0.0, 1.0, 0.25], "N"),
            (Boxes::NorthSouth, _) => ([0.0, 0.75, 1.0, 1.0], "S"),
            (Boxes::WestEast, 0) => ([0.0, 0.0, 0.25, 1.0], "W"),
            (Boxes::WestEast, _) => ([0.75, 0.0, 1.0, 1.0], "E"),
        }
    }

    /// Ally teams this layout has room for.
    pub fn capacity(self) -> usize {
        if self == Boxes::Corners { 4 } else { 2 }
    }
}

pub struct MatchSetup<'a> {
    /// The game's full name, not a rapid tag: the lobby reads it from a replay's header and cannot resolve a tag.
    pub game: &'a str,
    pub map: &'a str,
    pub opponent_profile: &'a str,
    /// BARb's `disabledunits` option (`name+name`), empty for none: how its opening is pinned.
    pub opponent_disabled_units: &'a str,
    pub host_port: u16,
    pub autohost_port: u16,
    pub seed: u32,
    /// Seats of ours, BARb seats on our ally team, and BARb seats against us.
    pub ours: usize,
    pub allies: usize,
    pub enemies: usize,
    /// Every enemy seat is its own ally team (they fight each other too); otherwise they are one team.
    pub free_for_all: bool,
    pub boxes: Boxes,
    /// Our ally team is ally team 0 and takes the first start box; otherwise ally team 1 and the second one.
    pub we_are_first: bool,
    /// Ally team 0 takes the second box and ally team 1 the first, to tell the box's effect from the team slot's.
    pub swap_corners: bool,
    pub our_side: &'static str,
    /// Give the opponents our faction instead of the other one.
    pub mirror: bool,
}

struct Seat {
    ai: String,
    ally_team: usize,
    side: &'static str,
}

impl MatchSetup<'_> {
    pub fn our_ally_team(&self) -> u8 {
        if self.we_are_first { 0 } else { 1 }
    }

    /// The start box our ally team plays from, by name.
    pub fn our_box(&self) -> &'static str {
        self.boxes.rect(self.box_of(self.our_ally_team() as usize)).1
    }

    fn box_of(&self, ally_team: usize) -> usize {
        match ally_team {
            0 | 1 if self.swap_corners => 1 - ally_team,
            other => other,
        }
    }

    fn enemy_ally_teams(&self) -> usize {
        if self.free_for_all { self.enemies } else { 1 }
    }

    pub fn render(&self) -> String {
        let ours = "ShortName=WReason; Version=0.1;".to_string();
        // `random_seed` is read by BARb (CircuitAI.cpp) though the lobby does not offer it. It does NOT make BARb
        // repeatable: it draws from the C library's `rand()`, which the whole engine process shares, and the same seed
        // gave a bot lab in one run and a vehicle plant in the next. It is set so that the clock is at least not an input.
        let barb = format!("ShortName=BARb; Version=stable; [OPTIONS] {{ profile={}; random_seed={}; disabledunits={}; }}", self.opponent_profile, self.seed, self.opponent_disabled_units);
        let other_side = match (self.mirror, self.our_side) {
            (true, side) => side,
            (false, "Armada") => "Cortex",
            (false, _) => "Armada",
        };
        let us = self.our_ally_team() as usize;
        // Enemy ally teams take the numbers ours leaves free: 1 (or 0), then 2, 3.
        let enemy_ally_team = |nth: usize| if nth == 0 { 1 - us } else { nth + 1 };
        let mut seats: Vec<Seat> = Vec::new();
        seats.extend((0..self.ours).map(|_| Seat { ai: ours.clone(), ally_team: us, side: self.our_side }));
        // An allied BARb plays the other faction, so nothing of ours may assume an ally's units are our kind.
        seats.extend((0..self.allies).map(|_| Seat { ai: barb.clone(), ally_team: us, side: other_side }));
        seats.extend((0..self.enemies).map(|nth| Seat { ai: barb.clone(), ally_team: enemy_ally_team(if self.free_for_all { nth } else { 0 }), side: other_side }));
        // Teams in ally-team order, as the 1v1 script always had them (team 0 is ally team 0's).
        seats.sort_by_key(|seat| seat.ally_team);

        let ally_teams = 1 + self.enemy_ally_teams();
        let mut sections = String::new();
        for (team, seat) in seats.iter().enumerate() {
            sections += &format!("\t[AI{team}] {{ Name=ai{team}; Team={team}; Host=0; {} }}\n", seat.ai);
        }
        for (team, seat) in seats.iter().enumerate() {
            sections += &format!("\t[TEAM{team}] {{ TeamLeader=0; AllyTeam={}; Side={}; }}\n", seat.ally_team, seat.side);
        }
        for ally_team in 0..ally_teams {
            let [left, top, right, bottom] = self.boxes.rect(self.box_of(ally_team)).0;
            sections += &format!("\t[ALLYTEAM{ally_team}] {{ NumAllies=0; StartRectLeft={left}; StartRectTop={top}; StartRectRight={right}; StartRectBottom={bottom}; }}\n");
        }
        format!(
            "[GAME]
{{
	GameType={game};
	MapName={map};
	IsHost=1;
	HostIP=127.0.0.1;
	HostPort={host_port};
	AutohostIP=127.0.0.1;
	AutohostPort={autohost_port};
	MyPlayerName=arena;
	StartPosType=2;
	GameStartDelay=0;
	FixedRNGSeed={seed};
	NumPlayers=1;
	NumTeams={teams};
	NumAllyTeams={ally_teams};
	[PLAYER0] {{ Name=arena; Spectator=1; }}
{sections}}}
",
            game = self.game,
            map = self.map,
            host_port = self.host_port,
            autohost_port = self.autohost_port,
            seed = self.seed,
            teams = seats.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(ours: usize, allies: usize, enemies: usize, free_for_all: bool, we_are_first: bool) -> MatchSetup<'static> {
        MatchSetup {
            game: "g", map: "m", opponent_profile: "medium", opponent_disabled_units: "", host_port: 1, autohost_port: 2, seed: 3,
            ours, allies, enemies, free_for_all, boxes: Boxes::Corners, we_are_first, swap_corners: false, our_side: "Armada", mirror: false,
        }
    }

    #[test]
    fn one_against_one_is_the_script_it_always_was() {
        let script = setup(1, 0, 1, false, false).render();
        assert!(script.contains("[AI0] { Name=ai0; Team=0; Host=0; ShortName=BARb;"));
        assert!(script.contains("[AI1] { Name=ai1; Team=1; Host=0; ShortName=WReason;"));
        assert!(script.contains("[TEAM1] { TeamLeader=0; AllyTeam=1; Side=Armada; }"));
        assert!(script.contains("[ALLYTEAM1] { NumAllies=0; StartRectLeft=0.7;"));
        assert!(script.contains("NumTeams=2;") && script.contains("NumAllyTeams=2;"));
    }

    #[test]
    fn two_of_ours_and_an_ally_against_a_free_for_all() {
        let setup = setup(2, 1, 2, true, true);
        let script = setup.render();
        assert_eq!(script.matches("ShortName=WReason;").count(), 2);
        assert_eq!(script.matches("ShortName=BARb;").count(), 3);
        assert_eq!(script.matches("AllyTeam=0;").count(), 3);
        assert!(script.contains("AllyTeam=1;") && script.contains("AllyTeam=2;") && script.contains("NumAllyTeams=3;"));
        assert_eq!(setup.our_box(), "NW");
    }
}
