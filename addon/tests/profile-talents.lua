-- Profile capture contract, independent class/Legacy fixtures; no game UI required.
local events = {}
local FG = {
    Edition = "forever",
    T = function(s) return s end,
    On = function(_, event, fn) events[event] = fn end,
    CharacterKey = function() return "Rogue-Realm" end,
    PlayerPosition = function() return 2521, 62, 73 end,
    ProfessionSkill = function() return 0 end,
    Reputations = function() return {} end,
    QuestLog = {
        GetAllCompletedQuestIDs = function() return { 10 } end,
        GetNumQuestLogEntries = function() return 0 end,
    },
}
FactoruideDB = {}
UnitRace = function() return nil, nil, 1 end
UnitClass = function() return "Rogue", "ROGUE" end
UnitFactionGroup = function() return "Horde" end
UnitLevel = function() return 13 end
UnitXP = function() return 100 end
UnitXPMax = function() return 1000 end
GetXPExhaustion = function() return 0 end
IsResting = function() return false end
GetMoney = function() return 0 end
IsInInstance = function() return false end
GetInventoryItemID = function() return nil end
GetBindLocation = function() return nil end
UnitCharacterPoints = function() return 1 end
local now = 100
time = function() return now end
C_Timer = { NewTicker = function() end }
Constants = {
    LegacyConsts = {
        LEGACY_TREE_PROFESSIONS_ID = 1187,
        LEGACY_TREE_ADVENTURE_ID = 1188,
        LEGACY_TREE_PROGRESSION_ID = 1189,
        LEGACY_TREE_ADVENTURE_TALENTED_NODE_ID = 110298,
    },
}
local staged, missing, group = {}, nil, 1
local trees = { [100] = { 10, 11 }, [200] = { 20 }, [1187] = { 30 }, [1188] = { 110298 }, [1189] = { 40 } }
local ranks = { [10] = 2, [11] = 2, [20] = 1, [30] = 1, [110298] = 5, [40] = 0 }
C_SpecializationInfo = {
    GetActiveSpecGroup = function() return group end,
    GetCombatConfigIDForSpecGroup = function(g) return g * 100 end,
}
C_Spell = { GetSpellName = function(id) return "Spell " .. id end }
C_Traits = {
    ConfigHasStagedChanges = function(config) return staged[config] or false end,
    GetConfigInfo = function(config) return { treeIDs = { config } } end,
    GetConfigIDByTreeID = function(tree) return tree end,
    GetTreeNodes = function(tree) return tree ~= missing and trees[tree] or nil end,
    GetNodeInfo = function(_, id)
        return { currentRank = ranks[id], maxRanks = 5, activeEntry = { entryID = id + 1000 } }
    end,
    GetEntryInfo = function(_, id) return { definitionID = id + 10000 } end,
    GetDefinitionInfo = function(id) return { spellID = id + 100000 } end,
}
assert(loadfile("addon/Factoruide_Profile/Profile.lua"))("Factoruide_Profile", FG)
events.PLAYER_LOGIN()
local p = FactoruideDB.profiles["Rogue-Realm"]
assert(not p.errors, "complete snapshot")
assert(p.talents.source == "traits" and p.talents.active_group == 1 and p.talents.unspent == 1)
assert(#p.talents.ranks == 2 and p.talents.ranks[1].rank == 2)
assert(p.talents.ranks[1].spell == 111010 and p.talents.ranks[1].name == "Spell 111010")
assert(#p.legacy.ranks == 2 and p.legacy.talented_rank == 5)
local talents, legacy = p.talents, p.legacy
now = 200
staged[100], staged[1188] = true, true
FG.Profile:Snapshot()
assert(p.talents == talents and p.legacy == legacy, "staged edits do not replace committed observations")
assert(p.errors.talents and p.errors.legacy and p.talents.observed_at == 100)
assert(p.time == 200 and p.position.map == 2521, "other profile sections still save")
staged = {}
missing = 1189
FG.Profile:Snapshot()
assert(p.legacy == legacy and p.errors.legacy, "partial Legacy read is atomic across all three trees")
missing = nil
ranks[110298] = 0
FG.Profile:Snapshot()
assert(not p.errors and p.legacy.talented_rank == 0, "confirmed reset replaces previous ranks")
group = 2
FG.Profile:Snapshot()
assert(p.talents.active_group == 2 and #p.talents.ranks == 1 and p.talents.ranks[1].node == 20)
C_SpecializationInfo.GetCombatConfigIDForSpecGroup = function() return nil end
FG.Profile:Snapshot()
assert(p.errors.talents and p.talents.active_group == 2, "unloaded active configuration preserves prior snapshot")
FG.Edition = "classic"
GetNumTalentTabs = function() return 1 end
GetNumTalents = function() return 2 end
GetTalentInfo = function(_, i) return "Classic " .. i, nil, 1, i, i == 1 and 2 or 0, 5 end
FG.Profile:Snapshot()
assert(not p.errors and p.legacy == nil and p.talents.source == "classic")
assert(#p.talents.ranks == 1 and p.talents.ranks[1].rank == 2 and p.talents.ranks[1].spell == nil)
print("profile talents/Legacy: committed, staged, partial, reset, active group and Classic passed")
