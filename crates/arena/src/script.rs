//! Start script generation (`doc/StartScriptFormat.txt` in the engine repo).

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
    /// We take team 0 and the north-west start box; otherwise team 1 and the south-east one.
    pub we_are_first: bool,
    /// Team 0 starts south-east and team 1 north-west, to tell the corner's effect from the team slot's.
    pub swap_corners: bool,
    pub our_side: &'static str,
    /// Give the opponent our faction instead of the other one.
    pub mirror: bool,
}

const NORTH_WEST: &str = "StartRectLeft=0; StartRectTop=0; StartRectRight=0.3; StartRectBottom=0.3;";
const SOUTH_EAST: &str = "StartRectLeft=0.7; StartRectTop=0.7; StartRectRight=1; StartRectBottom=1;";

impl MatchSetup<'_> {
    pub fn our_ally_team(&self) -> u8 {
        if self.we_are_first { 0 } else { 1 }
    }

    pub fn render(&self) -> String {
        let ours = "ShortName=WReason; Version=0.1;".to_string();
        // `random_seed` is read by BARb (CircuitAI.cpp) though the lobby does not offer it. It does NOT make BARb
        // repeatable: it draws from the C library's `rand()`, which the whole engine process shares, and the same seed
        // gave a bot lab in one run and a vehicle plant in the next. It is set so that the clock is at least not an input.
        let theirs = format!("ShortName=BARb; Version=stable; [OPTIONS] {{ profile={}; random_seed={}; disabledunits={}; }}", self.opponent_profile, self.seed, self.opponent_disabled_units);
        let their_side = match (self.mirror, self.our_side) {
            (true, side) => side,
            (false, "Armada") => "Cortex",
            (false, _) => "Armada",
        };
        let (ai0, ai1, side0, side1) = if self.we_are_first {
            (ours, theirs, self.our_side, their_side)
        } else {
            (theirs, ours, their_side, self.our_side)
        };
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
	NumTeams=2;
	NumAllyTeams=2;
	[PLAYER0] {{ Name=arena; Spectator=1; }}
	[AI0] {{ Name=ai0; Team=0; Host=0; {ai0} }}
	[AI1] {{ Name=ai1; Team=1; Host=0; {ai1} }}
	[TEAM0] {{ TeamLeader=0; AllyTeam=0; Side={side0}; }}
	[TEAM1] {{ TeamLeader=0; AllyTeam=1; Side={side1}; }}
	[ALLYTEAM0] {{ NumAllies=0; {rect0} }}
	[ALLYTEAM1] {{ NumAllies=0; {rect1} }}
}}
",
            rect0 = if self.swap_corners { SOUTH_EAST } else { NORTH_WEST },
            rect1 = if self.swap_corners { NORTH_WEST } else { SOUTH_EAST },
            game = self.game,
            map = self.map,
            host_port = self.host_port,
            autohost_port = self.autohost_port,
            seed = self.seed,
        )
    }
}
