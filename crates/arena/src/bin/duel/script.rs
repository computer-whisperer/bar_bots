//! Start script for a duel match: both teams are our AI, which only spawns and orders what the director tells it.

pub fn render(game: &str, map: &str, host_port: u16, autohost_port: u16, seed: u32) -> String {
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
	[AI0] {{ Name=ai0; Team=0; Host=0; ShortName=WReason; Version=0.1; }}
	[AI1] {{ Name=ai1; Team=1; Host=0; ShortName=WReason; Version=0.1; }}
	[TEAM0] {{ TeamLeader=0; AllyTeam=0; Side=Armada; }}
	[TEAM1] {{ TeamLeader=0; AllyTeam=1; Side=Cortex; }}
	[ALLYTEAM0] {{ NumAllies=0; StartRectLeft=0; StartRectTop=0; StartRectRight=0.3; StartRectBottom=0.3; }}
	[ALLYTEAM1] {{ NumAllies=0; StartRectLeft=0.7; StartRectTop=0.7; StartRectRight=1; StartRectBottom=1; }}
}}
"
    )
}
