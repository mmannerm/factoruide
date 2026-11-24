-- Isolated runtime test. The fixture is the requested 0/31/20 plan; the live-tree stub
-- independently supplies Forever rank caps, row gates and prerequisite relationships.
local name, class, locale = "Example Shield-US", "WARRIOR", "enUS"
local points, combat, forcedPrereq, noTree = 1, false, false, false
local errors, learned = {}, {}
local settings = { talentAdvisor = true, autoLearnTalents = false }
local tree = {
    { "Improved Heroic Strike", 1, 1, 3 },
    { "Booming Voice", 2, 1, 5 },
    { "Cruelty", 2, 1, 5 },
    { "Unbridled Wrath", 2, 2, 5 },
    { "Piercing Howl", 2, 3, 1 },
    { "Blood Craze", 2, 3, 3 },
    { "Enrage", 2, 3, 5 },
    { "Improved Berserker Rage", 2, 5, 2 },
    { "Death Wish", 2, 5, 1 },
    { "Gore Drinker", 2, 5, 2, "Blood Craze", 3 },
    { "Flurry", 2, 6, 3, "Enrage", 5 },
    { "Bloodthirst", 2, 7, 1, "Death Wish", 1 },
    { "Improved Bloodrage", 3, 1, 2 },
    { "Shield Specialization", 3, 1, 5 },
    { "Anticipation", 3, 2, 5 },
    { "Improved Revenge", 3, 2, 3 },
    { "Improved Thunder Clap", 3, 2, 3 },
    { "Last Stand", 3, 3, 1, "Improved Bloodrage", 2 },
    { "Master of Defense", 3, 3, 2, "Shield Specialization", 5 },
}
local byName, tabs = {}, { {}, {}, {} }
for _, t in ipairs(tree) do
    byName[t[1]] = t
    t.rank = 0
    -- Deliberately unrelated to cached Classic coordinates. Custom plans resolve names.
    t.column = 1
    for _, prior in ipairs(tabs[t[2]]) do
        if prior[3] == t[3] then t.column = t.column + 1 end
    end
    table.insert(tabs[t[2]], t)
end
local function spent(tab)
    local n = 0
    for _, t in ipairs(tabs[tab]) do
        n = n + t.rank
    end
    return n
end
local function reset()
    for _, t in ipairs(tree) do
        t.rank = 0
    end
    learned = {}
end
local plan = assert(loadfile("addon/tests/fixtures/fury-protection.lua"))()
local FG = {
    Edition = "forever",
    T = function(s) return s end,
    CharacterKey = function() return name end,
    Get = function(_, k) return settings[k] end,
    Print = function(_, text, message) table.insert(errors, message or text) end,
    On = function() end,
    CharacterTalentBuilds = { forever = { ["Example Shield-US"] = plan } },
    TalentBuilds = { forever = { WARRIOR = { picks = { { 1, 1, 1 } } } } },
}
local env = setmetatable({
    Factoruide = FG,
    GetLocale = function() return locale end,
    UnitClass = function() return class, class end,
    GetNumTalentTabs = function() return noTree and 0 or 3 end,
    GetNumTalents = function(tab) return #tabs[tab] end,
    GetTalentInfo = function(tab, index)
        local t = tabs[tab][index]
        return t[1], "icon", t[3], t.column, t.rank, t[4]
    end,
    GetTalentPrereqs = function(tab, index)
        if forcedPrereq then return 1, 1, 0 end
        local t = tabs[tab][index]
        if t[5] then
            local p = byName[t[5]]
            return p[3], p.column, p.rank >= t[6] and 1 or nil
        end
    end,
    UnitCharacterPoints = function() return points end,
    InCombatLockdown = function() return combat end,
    LearnTalent = function(tab, index)
        local t = tabs[tab][index]
        assert(points > 0 and not combat)
        assert(t.rank < t[4] and spent(tab) >= (t[3] - 1) * 5, "illegal row/rank: " .. t[1])
        assert(not t[5] or byName[t[5]].rank >= t[6], "illegal prerequisite: " .. t[1])
        t.rank = t.rank + 1
        points = points - 1
        table.insert(learned, t[1])
    end,
}, { __index = _G })
local chunk = assert(loadfile("addon/Factoruide_Guide/Talents.lua"))
setfenv(chunk, env)
chunk()
local override = assert(loadfile("addon/Factoruide_CharacterTalents/Runtime.lua"))
setfenv(override, env)
override()

assert(FG.Talents:Next().name == "Shield Specialization")
settings.talentAdvisor = false
assert(FG.Talents:Pending() == nil)
settings.talentAdvisor = true
points = 0
assert(FG.Talents:Pending() == nil and not FG.Talents:LearnNext())
points, combat = 1, true
assert(not FG.Talents:LearnNext())
combat = false
FG.Talents:Refresh()
assert(#learned == 0, "must not enable auto-learning")
for level = 10, 60 do
    points = 1
    assert(FG.Talents:LearnNext(), "missing suggestion at level " .. level)
    if level == 21 then assert(byName["Master of Defense"].rank == 2) end
    if level == 29 then assert(spent(3) == 20 and spent(2) == 0) end
end
local expected = {
    ["Cruelty"] = 5,
    ["Booming Voice"] = 3,
    ["Unbridled Wrath"] = 5,
    ["Enrage"] = 5,
    ["Blood Craze"] = 3,
    ["Piercing Howl"] = 1,
    ["Gore Drinker"] = 2,
    ["Death Wish"] = 1,
    ["Improved Berserker Rage"] = 2,
    ["Flurry"] = 3,
    ["Bloodthirst"] = 1,
    ["Shield Specialization"] = 5,
    ["Improved Bloodrage"] = 2,
    ["Anticipation"] = 5,
    ["Improved Revenge"] = 3,
    ["Improved Thunder Clap"] = 2,
    ["Last Stand"] = 1,
    ["Master of Defense"] = 2,
}
assert(#learned == 51 and spent(1) == 0 and spent(2) == 31 and spent(3) == 20)
for _, t in ipairs(tree) do
    assert(t.rank == (expected[t[1]] or 0), t[1])
end
assert(FG.Talents:Next() == nil and learned[51] == "Bloodthirst")
reset()
points, settings.autoLearnTalents = 5, true
FG.Talents:Refresh()
assert(#learned == 5 and byName["Shield Specialization"].rank == 5)
settings.autoLearnTalents = false
reset()

-- Same first name, another surname; another region; another edition/class: no override.
for _, other in ipairs({ "Example Other-US", "Example Shield-EU", "Someone Else-US" }) do
    name = other
    assert(FG.Talents:Next().name == "Improved Heroic Strike")
end
name = "Example Shield-US"
FG.Edition = "classic"
FG.TalentBuilds.classic = FG.TalentBuilds.forever
assert(FG.Talents:Next().name == "Improved Heroic Strike")
FG.Edition = "forever"
class = "MAGE"
FG.TalentBuilds.forever.MAGE = { picks = { { 1, 1, 1 } } }
assert(FG.Talents:Next().name == "Improved Heroic Strike")
class = "WARRIOR"
locale = "frFR"
assert(FG.Talents:Next() == nil, "locale mismatch must not fall back to Arms")
locale = "enUS"
noTree = true
assert(FG.Talents:Next() == nil)
noTree = false
byName["Gore Drinker"][4] = 1
assert(FG.Talents:Next() == nil, "validate later ranks before spending first point")
byName["Gore Drinker"][4] = 2
byName["Gore Drinker"][1] = "Changed Talent"
assert(FG.Talents:Next() == nil, "missing talent must not fall back")
byName["Gore Drinker"][1] = "Gore Drinker"
byName["Improved Heroic Strike"].rank = 1
assert(FG.Talents:Next() == nil, "off-plan points require explicit review")
reset()
forcedPrereq = true
assert(FG.Talents:Next() == nil, "numeric zero means prerequisite unavailable")
forcedPrereq = false
byName["Shield Specialization"][3] = 2
assert(FG.Talents:Next() == nil, "live row gates must be checked")
byName["Shield Specialization"][3] = 1
assert(FG.Talents:Next().name == "Shield Specialization")
assert(settings.talentAdvisor == true and settings.autoLearnTalents == false)
print("character talents ok: exact 0/31/20, 51 legal points, isolated defaults and fail-closed updates")
