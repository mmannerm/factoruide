-- Optional character-specific talent advice. Keep the stock class plans and settings intact.
local FG = Factoruide
local Talents = assert(FG.Talents, "Factoruide_Guide talent advisor is required")
local ClassNext = assert(Talents.Next, "Factoruide_Guide talent advisor API changed")

--- Talents of the character: "tab:tier:column" -> { tab, index, name, icon, rank, maxRank }.
local function Known()
    local out = {}
    for tab = 1, (GetNumTalentTabs and GetNumTalentTabs() or 0) do
        for i = 1, GetNumTalents(tab) do
            local name, icon, tier, column, rank, maxRank = GetTalentInfo(tab, i)
            if name then
                out[tab .. ":" .. tier .. ":" .. column] =
                    { tab = tab, index = i, name = name, icon = icon, rank = rank, maxRank = maxRank }
            end
        end
    end
    return out
end

-- Optional character plans come from a separate local addon, never from a route or an
-- account-wide setting. Names are in the declared client locale and resolved against the
-- live tree: cached Classic coordinates must not choose different Forever talents.
local lastPlanError
local function PlanError(message)
    if lastPlanError ~= message then
        FG:Print("Character talent plan: %s", message)
        lastPlanError = message
    end
    return nil
end

local function CharacterNext(build, known)
    if build.locale ~= GetLocale() then return PlanError("client locale does not match the plan") end
    if not next(known) then return nil end -- the talent tree has not loaded yet
    local talents, spent, wanted = {}, {}, {}
    for _, talent in pairs(known) do
        local key = talent.tab .. ":" .. talent.name
        talents[key] = talent
        spent[talent.tab] = (spent[talent.tab] or 0) + talent.rank
    end
    -- Check the entire plan before suggesting or spending even its first point. A client
    -- update may remove a talent or change its rank cap; never fall back to the class plan.
    for _, pick in ipairs(build.picks) do
        local key = pick[1] .. ":" .. pick[2]
        wanted[key] = (wanted[key] or 0) + 1
        local talent = talents[key]
        if not talent then return PlanError("talent not found: " .. pick[2]) end
        if wanted[key] > talent.maxRank then return PlanError("rank limit changed: " .. pick[2]) end
    end
    for key, talent in pairs(talents) do
        if talent.rank > (wanted[key] or 0) then
            return PlanError("learned talents differ from the plan; review them before spending more points")
        end
    end
    local ranks = {}
    for _, pick in ipairs(build.picks) do
        local key = pick[1] .. ":" .. pick[2]
        ranks[key] = (ranks[key] or 0) + 1
        local talent = talents[key]
        if talent.rank < ranks[key] then
            local _, _, tier = GetTalentInfo(talent.tab, talent.index)
            if (spent[talent.tab] or 0) < (tier - 1) * 5 then
                return PlanError("not enough points in the tree for " .. talent.name)
            end
            local prereqs = { GetTalentPrereqs(talent.tab, talent.index) }
            for i = 1, #prereqs, 3 do
                if prereqs[i + 2] ~= true and prereqs[i + 2] ~= 1 then
                    return PlanError("prerequisite not learned for " .. talent.name)
                end
            end
            lastPlanError = nil
            return talent
        end
    end
    lastPlanError = nil
    return nil
end

function Talents:Next()
    local overrides = FG.CharacterTalentBuilds and FG.CharacterTalentBuilds[FG.Edition]
    local character = overrides and overrides[FG:CharacterKey()]
    if character and character.class == select(2, UnitClass("player")) then return CharacterNext(character, Known()) end
    return ClassNext(self)
end
