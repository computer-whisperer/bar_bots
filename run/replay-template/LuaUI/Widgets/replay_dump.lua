-- Within Reason: while the engine plays a demo, write every team's game in the match record format
-- (docs/harness/record-format.md) as record-<team>.jsonl, and every other team's units as truth-<team>.jsonl, so the
-- measurement scripts read a human's game as they read ours. Loaded by run/replay_match.py; does nothing in a live game.
local widget = widget

function widget:GetInfo()
	return { name = "Replay Dump", desc = "writes match records for every team of a replay", layer = 0, enabled = true }
end

local SAMPLE = 30
local TRUTH = 60
local teams = {}
local files = {}
local truths = {}
local defIndex = {}
local defRows = {}
local headerDone = false

local function esc(s)
	return '"' .. tostring(s):gsub('[%c"\\]', function(c) return string.format("\\u%04x", c:byte()) end) .. '"'
end

local function json(v)
	local t = type(v)
	if t == "table" then
		if #v > 0 or next(v) == nil then
			local parts = {}
			for i = 1, #v do parts[i] = json(v[i]) end
			return "[" .. table.concat(parts, ",") .. "]"
		end
		local keys = {}
		for k in pairs(v) do keys[#keys + 1] = k end
		table.sort(keys)
		local parts = {}
		for i, k in ipairs(keys) do parts[i] = esc(k) .. ":" .. json(v[k]) end
		return "{" .. table.concat(parts, ",") .. "}"
	elseif t == "number" then
		if v ~= v or v == math.huge or v == -math.huge then return "null" end
		if v == math.floor(v) then return string.format("%d", v) end
		return string.format("%.4g", v)
	elseif t == "string" then
		return esc(v)
	elseif t == "boolean" then
		return v and "true" or "false"
	end
	return "null"
end

local function class(d)
	local mobile = (d.speed or 0) > 0
	local builds = d.buildOptions and #d.buildOptions > 0
	if d.name:sub(-3) == "com" and mobile and (d.buildSpeed or 0) > 0 then return "commander" end
	if (d.extractsMetal or 0) > 0 then return "extractor" end
	if not mobile and builds then return "factory" end
	if not mobile and #d.weapons > 0 then return "turret" end
	if not mobile then return "building" end
	if (d.buildSpeed or 0) > 0 then return "builder" end
	if #d.weapons > 0 then return "army" end
	return "other"
end

local function put(team, line)
	local f = files[team]
	if f then f:write(line, "\n") end
end

local function header(team)
	-- The record's first line, before any event of frame 0: the side from the team's setup, not from a unit.
	local _, _, _, _, sideName, allyTeam = Spring.GetTeamInfo(team)
	local side = (sideName or ""):sub(1, 3)
	local spots = {}
	put(team, json({
		t = "header", format = "within-reason-record", version = 1, ai_id = team, team = team, ally_team = allyTeam, side = side,
		mode = "replay", start_frame = 0, first_tick_frame = 0, frames_per_second = 30, sample_frames = SAMPLE,
		wall_start = os.time(),
		map = { name = Game.mapName, width = Game.mapSizeX, height = Game.mapSizeZ, wind_min = Game.windMin, wind_max = Game.windMax },
		grid = { columns = 8, rows = 8 }, metal_spots = spots, unit_defs = defRows,
		siblings = { bot_log = "", engine_log = "infolog.txt", replay = "*.sdfz", decision_logs = {} },
	}))
end

function widget:Initialize()
	if not Spring.IsReplay() then
		widgetHandler:RemoveWidget(self)
		return
	end
	local ids = {}
	for id in pairs(UnitDefs) do ids[#ids + 1] = id end
	table.sort(ids)
	for i, id in ipairs(ids) do
		local d = UnitDefs[id]
		defIndex[id] = i - 1
		defRows[i] = { id = id, name = d.name, class = class(d), metal = d.metalCost, energy = d.energyCost, speed = d.speed, weapons = #d.weapons }
	end
	local gaia = Spring.GetGaiaTeamID()
	for _, team in ipairs(Spring.GetTeamList()) do
		if team ~= gaia then
			teams[#teams + 1] = team
			files[team] = io.open("record-" .. team .. ".jsonl", "w")
			truths[team] = io.open("truth-" .. team .. ".jsonl", "w")
		end
	end
	for _, team in ipairs(teams) do header(team) end
	headerDone = true
	Spring.Echo("<replay dump> " .. #teams .. " teams")
end

-- Played at its own pace a demo takes as long as the game did, waiting through the lobby's countdown and every pause;
-- the local server's skip reads it as fast as the client simulates, and past the end just stops skipping. Sent once
-- the game has started: sent at load, a demo with a long countdown hung the client before its first frame.
function widget:GameStart()
	if Spring.IsReplay() then
		Spring.SendCommands("skip f100000000")
	end
end

local function unitRow(unitID, withMax)
	local defID = Spring.GetUnitDefID(unitID)
	local x, _, z = Spring.GetUnitPosition(unitID)
	local health, maxHealth, _, _, progress = Spring.GetUnitHealth(unitID)
	local flags = 0
	if (progress or 1) < 1 then flags = flags + 1 end
	if Spring.GetCommandQueue(unitID, 0) == 0 then flags = flags + 2 end
	local pct = (health and maxHealth and maxHealth > 0) and math.floor(health / maxHealth * 100) or 0
	return unitID, defID, math.floor(x or 0), math.floor(z or 0), pct, flags, health or 0, (progress or 1) < 1
end

function widget:GameFrame(frame)
	if frame % SAMPLE ~= 0 then return end
	local rows = {}
	for _, team in ipairs(teams) do
		rows[team] = {}
		for _, unitID in ipairs(Spring.GetTeamUnits(team)) do
			local id, defID, x, z, pct, flags, health, building = unitRow(unitID)
			rows[team][#rows[team] + 1] = { id, defID, x, z, pct, flags, health, building }
		end
	end
	for _, team in ipairs(teams) do
		local own, en = {}, {}
		for _, other in ipairs(teams) do
			for _, r in ipairs(rows[other]) do
				if other == team then
					own[#own + 1] = { r[1], defIndex[r[2]] or -1, r[3], r[4], r[5], r[6] }
				elseif not Spring.AreTeamsAllied(team, other) then
					en[#en + 1] = { r[1], defIndex[r[2]] or -1, r[3], r[4], r[7] }
				end
			end
		end
		local mCur, mStore, _, mInc, mExp = Spring.GetTeamResources(team, "metal")
		local eCur, eStore, _, eInc, eExp = Spring.GetTeamResources(team, "energy")
		put(team, json({ t = "s", f = frame, m = { mCur or 0, mInc or 0, mExp or 0, mStore or 0 }, e = { eCur or 0, eInc or 0, eExp or 0, eStore or 0 }, own = own, en = en, al = {}, dmg = {}, ms = 0 }))
		if frame % TRUTH == 0 and truths[team] then
			local enemy = {}
			for _, other in ipairs(teams) do
				if not Spring.AreTeamsAllied(team, other) then
					for _, r in ipairs(rows[other]) do
						enemy[#enemy + 1] = { r[1], UnitDefs[r[2]].name, r[3], r[4], r[5], r[8] and 1 or 0 }
					end
				end
			end
			truths[team]:write(json({ f = frame, enemy = enemy }), "\n")
		end
		files[team]:flush()
	end
end

local function place(unitID)
	local x, _, z = Spring.GetUnitPosition(unitID)
	return math.floor(x or 0), math.floor(z or 0)
end

function widget:UnitCreated(unitID, unitDefID, unitTeam, builderID)
	local x, z = place(unitID)
	put(unitTeam, json({ t = "ev", f = Spring.GetGameFrame(), k = "created", u = unitID, d = defIndex[unitDefID] or -1, x = x, z = z, by = builderID }))
end

function widget:UnitFinished(unitID, unitDefID, unitTeam)
	local x, z = place(unitID)
	put(unitTeam, json({ t = "ev", f = Spring.GetGameFrame(), k = "finished", u = unitID, d = defIndex[unitDefID] or -1, x = x, z = z }))
end

function widget:UnitDestroyed(unitID, unitDefID, unitTeam, attackerID, attackerDefID, attackerTeam)
	local x, z = place(unitID)
	local frame = Spring.GetGameFrame()
	for _, team in ipairs(teams) do
		if team == unitTeam then
			put(team, json({ t = "ev", f = frame, k = "destroyed", u = unitID, d = defIndex[unitDefID] or -1, x = x, z = z, by = attackerID, by_d = attackerDefID and defIndex[attackerDefID] or -1 }))
		elseif not Spring.AreTeamsAllied(team, unitTeam) then
			put(team, json({ t = "ev", f = frame, k = "enemy_destroyed", u = unitID, d = defIndex[unitDefID] or -1, x = x, z = z }))
		end
	end
end

local NAMES = { [CMD.STOP] = "stop", [CMD.MOVE] = "move", [CMD.FIGHT] = "fight", [CMD.ATTACK] = "attack", [CMD.GUARD] = "guard", [CMD.REPAIR] = "repair", [CMD.RECLAIM] = "reclaim", [CMD.RESURRECT] = "resurrect", [CMD.PATROL] = "patrol" }

function widget:UnitCommand(unitID, unitDefID, unitTeam, cmdID, cmdParams, cmdOpts, cmdTag)
	local row
	if cmdID < 0 then
		row = { "build", unitID, defIndex[-cmdID] or -1, math.floor(cmdParams[1] or 0), math.floor(cmdParams[3] or 0) }
	elseif NAMES[cmdID] then
		row = { NAMES[cmdID], unitID }
		for i, p in ipairs(cmdParams) do row[#row + 1] = math.floor(p) end
	else
		row = { "other:" .. cmdID, unitID }
	end
	put(unitTeam, json({ t = "cmd", f = Spring.GetGameFrame(), c = { row } }))
end

function widget:GameOver()
	for _, team in ipairs(teams) do
		put(team, json({ t = "result", f = Spring.GetGameFrame(), result = { outcome = "replay" }, opponent = "replay", replay = "*.sdfz" }))
		if files[team] then files[team]:close() end
		if truths[team] then truths[team]:close() end
	end
	files, truths = {}, {}
end

function widget:Shutdown()
	for team, f in pairs(files) do f:close() end
	for team, f in pairs(truths) do f:close() end
end
