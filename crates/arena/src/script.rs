//! Start script generation (`doc/StartScriptFormat.txt` in the engine repo).

pub struct MatchSetup<'a> {
    pub map: &'a str,
    pub opponent_profile: &'a str,
    pub host_port: u16,
    pub autohost_port: u16,
    pub seed: u32,
    /// We take team 0 and the north-west start box; otherwise team 1 and the south-east one.
    pub we_are_first: bool,
    pub our_side: &'static str,
}

impl MatchSetup<'_> {
    pub fn our_ally_team(&self) -> u8 {
        if self.we_are_first { 0 } else { 1 }
    }

    pub fn render(&self) -> String {
        let ours = "ShortName=BarBots; Version=0.1;".to_string();
        let theirs = format!("ShortName=BARb; Version=stable; [OPTIONS] {{ profile={}; }}", self.opponent_profile);
        let their_side = if self.our_side == "Armada" { "Cortex" } else { "Armada" };
        let (ai0, ai1, side0, side1) = if self.we_are_first {
            (ours, theirs, self.our_side, their_side)
        } else {
            (theirs, ours, their_side, self.our_side)
        };
        format!(
            "[GAME]
{{
	GameType=byar:test;
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
	[ALLYTEAM0] {{ NumAllies=0; StartRectLeft=0; StartRectTop=0; StartRectRight=0.3; StartRectBottom=0.3; }}
	[ALLYTEAM1] {{ NumAllies=0; StartRectLeft=0.7; StartRectTop=0.7; StartRectRight=1; StartRectBottom=1; }}
}}
",
            map = self.map,
            host_port = self.host_port,
            autohost_port = self.autohost_port,
            seed = self.seed,
        )
    }
}
