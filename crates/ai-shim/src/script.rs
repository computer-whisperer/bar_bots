//! The little we read from the game's start script (`doc/StartScriptFormat.txt` in the engine repo): each ally
//! team's start box. The AI interface has no call for it.

/// `(ally team, left, top, right, bottom)` as fractions of the map, for every `[ALLYTEAMn]` section that has a box.
pub fn start_rects(script: &str) -> Vec<(i32, [f32; 4])> {
    let lower = script.to_ascii_lowercase();
    let mut rects = Vec::new();
    let mut rest = lower.as_str();
    while let Some(at) = rest.find("[allyteam") {
        rest = &rest[at + "[allyteam".len()..];
        let Some(close) = rest.find(']') else { break };
        let Ok(ally_team) = rest[..close].trim().parse::<i32>() else { continue };
        let Some(open) = rest.find('{') else { break };
        let body = &rest[open + 1..rest[open..].find('}').map_or(rest.len(), |end| open + end)];
        let value = |key: &str| {
            body.split(';').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (k.trim() == key).then(|| v.trim().parse::<f32>().ok()).flatten()
            })
        };
        if let (Some(left), Some(top), Some(right), Some(bottom)) =
            (value("startrectleft"), value("startrecttop"), value("startrectright"), value("startrectbottom"))
        {
            rects.push((ally_team, [left, top, right, bottom]));
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_boxes_and_skips_ally_teams_without_one() {
        let script = "[GAME]\n{\n[TEAM0] { AllyTeam=0; }\n[ALLYTEAM0] { NumAllies=0; StartRectLeft=0; StartRectTop=0; StartRectRight=0.3; StartRectBottom=0.25; }\n[allyteam1]\n{\nnumallies=0;\n}\n[ALLYTEAM2] { StartRectLeft=0.7; StartRectTop=0.7; StartRectRight=1; StartRectBottom=1; }\n}";
        assert_eq!(super::start_rects(script), vec![(0, [0.0, 0.0, 0.3, 0.25]), (2, [0.7, 0.7, 1.0, 1.0])]);
    }
}
