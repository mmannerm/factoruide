-- Independent live-tree bounds; do not derive legality or final allocations from presets.
local trees = {
    ROGUE = {
        { "Efficient Strikes", 2, 1, 2 },
        { "Coup de Grace", 2, 1, 3 },
        { "Deflection", 2, 2, 3 },
        { "Precision", 2, 2, 3 },
        { "Riposte", 2, 3, 1, "Deflection", 3 },
        { "Endurance", 2, 3, 2 },
        { "Composure", 2, 3, 2 },
        { "Flawless Execution", 2, 4, 1 },
        { "Weapon Expertise", 2, 4, 2, "Precision", 3 },
        { "Aggression", 2, 4, 3 },
        { "Blade Flurry", 2, 5, 1 },
        { "Dual Wield Specialization", 2, 5, 2 },
        { "Hack and Slash", 2, 5, 5 },
        { "Flash of Steel", 2, 6, 5 },
        { "Adrenaline Rush", 2, 7, 1, "Blade Flurry", 1 },
        { "Malice", 1, 1, 5 },
        { "Ruthlessness", 1, 2, 3 },
        { "Improved Slice and Dice", 1, 2, 3 },
        { "Lethality", 1, 3, 2, "Malice", 5 },
        { "Vigor", 1, 3, 2 },
        { "Blood Splatter", 1, 3, 1 },
        { "Cold Blood", 1, 4, 1 },
    },
    MAGE = {
        { "Wand Specialization", 1, 1, 2 },
        { "Improved Channeling", 1, 1, 5 },
        { "Missile Barrage", 1, 2, 2 },
        { "Arcane Subtlety", 1, 2, 2 },
        { "Arcane Concentration", 1, 2, 5 },
        { "Arcane Blast", 1, 3, 1 },
        { "Arcane Meditation", 1, 4, 3 },
        { "Improved Frostbolt", 3, 1, 5 },
        { "Precision", 3, 1, 3 },
        { "Ice Shards", 3, 2, 5 },
        { "Frostbite", 3, 2, 3 },
        { "Permafrost", 3, 2, 3 },
        { "Ice Lance", 3, 3, 1 },
        { "Frost Channeling", 3, 3, 3 },
        { "Ice Block", 3, 4, 1 },
        { "Shatter", 3, 4, 3, "Ice Block", 1 },
        { "Arctic Reach", 3, 4, 2 },
        { "Cold Snap", 3, 5, 1 },
        { "Fingers of Frost", 3, 5, 2, "Ice Lance", 1 },
        { "Improved Cone of Cold", 3, 5, 3 },
        { "Ice Barrier", 3, 7, 1, "Cold Snap", 1 },
    },
}
local function run(class, preset)
    local level, points, learned = 10, 0, 0
    local messages, tabs, byName = {}, { {}, {}, {} }, {}
    for _, t in ipairs(trees[class]) do
        t.rank, t.column = 0, 1
        for _, prior in ipairs(tabs[t[2]]) do
            if prior[3] == t[3] then t.column = t.column + 1 end
        end
        table.insert(tabs[t[2]], t)
        byName[t[1]] = t
    end
    local function spent(tab)
        local n = 0
        for _, t in ipairs(tabs[tab]) do
            n = n + t.rank
        end
        return n
    end
    local settings = { talentAdvisor = true, autoLearnTalents = false }
    local FG = {
        Edition = "forever",
        T = function(s) return s end,
        CharacterKey = function() return "Example Character-US" end,
        Get = function(_, k) return settings[k] end,
        Print = function(_, _, message) table.insert(messages, message) end,
        On = function() end,
    }
    local env = setmetatable({
        Factoruide = FG,
        UnitLevel = function() return level end,
        GetLocale = function() return "enUS" end,
        UnitClass = function() return class, class end,
        GetNumTalentTabs = function() return 3 end,
        GetNumTalents = function(tab) return #tabs[tab] end,
        GetTalentInfo = function(tab, index)
            local t = tabs[tab][index]
            return t[1], "icon", t[3], t.column, t.rank, t[4]
        end,
        GetTalentPrereqs = function(tab, index)
            local t = tabs[tab][index]
            if t[5] then
                local prereq = byName[t[5]]
                return prereq[3], prereq.column, prereq.rank >= t[6] and 1 or nil
            end
        end,
        UnitCharacterPoints = function() return points end,
        InCombatLockdown = function() return false end,
        LearnTalent = function(tab, index)
            local t = tabs[tab][index]
            assert(points > 0 and t.rank < t[4], "invalid point: " .. t[1])
            assert(spent(tab) >= (t[3] - 1) * 5, "row gate: " .. t[1])
            assert(not t[5] or byName[t[5]].rank >= t[6], "prerequisite: " .. t[1])
            t.rank, points, learned = t.rank + 1, points - 1, learned + 1
        end,
        ConfirmTalentWipe = function() error("must never reset talents") end,
        LearnPreviewTalents = function() error("must never reset talents") end,
    }, { __index = _G })
    local function load(path)
        local chunk = assert(loadfile(path))
        setfenv(chunk, env)
        chunk()
    end
    load("addon/Factoruide_Guide/Talents.lua")
    load("addon/Factoruide_CharacterTalents/Presets.lua")
    assert(FG.CharacterTalentBuilds == nil, "presets must not bind characters")
    FG.CharacterTalentBuilds =
        { forever = {
            ["Example Character-US"] = FG.CharacterTalentPresets.forever[preset],
        } }
    load("addon/Factoruide_CharacterTalents/Runtime.lua")
    for at = 10, 60 do
        level, points = at, 1
        if class == "MAGE" and at == 40 then
            assert(spent(1) == 7 and spent(2) == 0 and spent(3) == 23)
            local before = learned
            settings.autoLearnTalents = true
            FG.Talents:Refresh()
            assert(learned == before and points == 1, "must pause at phase change")
            assert(messages[#messages]:find("reset talents manually", 1, true))
            -- Simulate the user resetting at a trainer, without reloading the addon.
            for _, t in pairs(byName) do
                t.rank = 0
            end
            points = 31
            FG.Talents:Refresh()
            assert(points == 0 and spent(1) == 0 and spent(3) == 31)
            assert(byName["Ice Barrier"].rank == 1)
            settings.autoLearnTalents = false
        else
            FG.Talents:Refresh()
            assert(points == 1, "must preserve disabled auto-learning")
            assert(FG.Talents:LearnNext(), class .. " missing suggestion at " .. at)
        end
        if class == "ROGUE" then
            if at == 20 then assert(byName.Riposte.rank == 1) end
            if at == 30 then assert(byName["Blade Flurry"].rank == 1) end
            if at == 40 then assert(byName["Adrenaline Rush"].rank == 1) end
        elseif at == 18 then
            assert(byName["Missile Barrage"].rank == 2)
        elseif at == 32 then
            assert(byName["Ice Block"].rank == 1)
        end
    end
    assert(FG.Talents:Next() == nil and points == 0)
    assert(settings.talentAdvisor and not settings.autoLearnTalents)
    if class == "ROGUE" then
        assert(learned == 51 and spent(1) == 16 and spent(2) == 35 and spent(3) == 0)
        assert(byName["Cold Blood"].rank == 1 and byName["Flash of Steel"].rank == 4)
    else
        assert(learned == 81 and spent(1) == 18 and spent(2) == 0 and spent(3) == 33)
        local expected = {
            ["Improved Channeling"] = 5,
            ["Missile Barrage"] = 2,
            ["Arcane Subtlety"] = 2,
            ["Arcane Concentration"] = 5,
            ["Arcane Blast"] = 1,
            ["Arcane Meditation"] = 3,
            ["Improved Frostbolt"] = 5,
            ["Precision"] = 3,
            ["Ice Shards"] = 5,
            ["Permafrost"] = 3,
            ["Frost Channeling"] = 3,
            ["Ice Lance"] = 1,
            ["Arctic Reach"] = 2,
            ["Ice Block"] = 1,
            ["Shatter"] = 3,
            ["Improved Cone of Cold"] = 3,
            ["Cold Snap"] = 1,
            ["Fingers of Frost"] = 2,
            ["Ice Barrier"] = 1,
        }
        for name, t in pairs(byName) do
            assert(t.rank == (expected[name] or 0), name)
        end
    end
end
run("ROGUE", "rogue_combat")
run("MAGE", "mage_frost_arcane")
print("talent presets ok: legal Combat progression; Frost/Arcane pauses for manual reset at 40")
