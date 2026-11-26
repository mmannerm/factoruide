//! Which quests the planner considers: those of the race and class of a member of the group (the
//! character alone) and of its levels, then the rules that leave quests out.

use super::builder::{QuestBuilder, Skip};
use super::sources::QuestRow;

/// The profession a quest needs: None for no profession, Some(None) for one the profile does not
/// level, Some(Some((profession index, skill))) otherwise.
pub(super) type QuestSkill = Option<Option<(usize, i64)>>;

impl QuestBuilder<'_> {
    pub(super) fn concerns(&self, r: &QuestRow) -> bool {
        let profile = self.profile;
        let level = r.level.unwrap_or(1).max(1);
        let min_level = r.min_level.unwrap_or(1).max(1);
        // A quest of a member's own race and class (in a group, the others' too: done together,
        // their XP goes to those it is for).
        profile.members_of(r.races, r.classes) != 0
            && (!profile.required_quests.is_empty()
                || self.any_level
                || self.sources.class_quest_ids.contains(&r.id)
                || profile.within_levels(level, min_level, self.params.max_quest_above))
    }

    /// Profession quests: kept when the profile levels that profession, accepted once the skill
    /// is reached.
    pub(super) fn skill(&self, r: &QuestRow) -> QuestSkill {
        r.skill.filter(|s| *s != 0).map(|id| {
            (self.profile.profession_goals().into_iter())
                .filter_map(|(g, _)| crate::profession::find(&g.key))
                .position(|d| d.skill_id == id)
                .map(|p| (p, r.skill_value.unwrap_or(0)))
        })
    }

    pub(super) fn reputation_ceiling(&self, id: i64) -> Option<super::types::ReputationCeiling> {
        self.profile
            .quest_overrides
            .iter()
            .rev()
            .filter(|q| q.id == id)
            .find_map(|q| q.reputation_below)
    }

    /// Do not invent eligibility from an absent save, or grind reputation down to unlock a
    /// repair quest. An accepted quest keeps its objectives and turn-in even above the ceiling.
    fn check_reputation_ceiling(&self, r: &QuestRow) -> Result<(), Skip> {
        let Some(limit) = self.reputation_ceiling(r.id) else {
            return Ok(());
        };
        if self.starts.len() == self.profile.members().count()
            && self
                .starts
                .iter()
                .all(|s| s.completed.iter().any(|q| q.0 == r.id) || s.log.iter().any(|q| q.id.0 == r.id))
        {
            return Ok(());
        }
        // Runtime reputation tracks the leader only; a shared route cannot yet prove each
        // member remains below the ceiling after different racial reputation bonuses.
        if self.profile.members().count() > 1 {
            return Err("reputation ceiling needs individual character routes");
        }
        let standing = self
            .starts
            .first()
            .and_then(|s| s.reputation.iter().find(|rep| rep.id == limit.faction));
        match standing {
            Some(rep) if rep.value < limit.value => Ok(()),
            Some(_) => Err("reputation is too high to accept this quest"),
            None => Err("reputation below the pickup ceiling is not recorded"),
        }
    }

    pub(super) fn check_rules(&self, r: &QuestRow, skill: QuestSkill) -> Result<(), Skip> {
        if self.excluded.contains(&r.id) {
            return Err("excluded by configuration");
        }
        self.check_reputation_ceiling(r)?;
        let excluded_sort = r.sort.is_some_and(|s| s < 0 && self.excluded_sorts.contains(&-s));
        if excluded_sort && !matches!(skill, Some(Some(_))) {
            return Err("holiday/profession/event category");
        }
        if self.sources.pvp.contains(&r.id) && !self.params.pvp_quests {
            return Err("PvP (battleground) quest");
        }
        if self.sources.cloth_donations.contains(&r.id) && !self.params.cloth_donations {
            return Err("cloth donation");
        }
        // A negative spell is one the character must not know yet: the spell the quest teaches
        // (Camping 101's Camp Tent), never known before taking it.
        if matches!(skill, Some(None)) || r.spell.unwrap_or(0) > 0 {
            return Err("needs a profession, reputation or spell");
        }
        if let Some(faction) = r.rep.filter(|&f| f != 0) {
            if self.params.rival_clan_chosen(faction) {
                return Err("reputation with the other clan chosen");
            }
            if !self
                .sources
                .reputation_reachable(faction, r.rep_min.unwrap_or(0) as f64)
            {
                return Err("needs a reputation no kill gives");
            }
        }
        Ok(())
    }
}
