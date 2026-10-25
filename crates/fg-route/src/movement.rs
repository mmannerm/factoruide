//! Sustained travel abilities, independent of combat-power weights. An ability must be known
//! or learned at an actual training visit. Talents are never inferred from character level.

use crate::{Params, job::StartState};
use anyhow::Result;
use rusqlite::Connection;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MovementDef {
    pub class: String,
    pub name: String,
    pub spells: Vec<i64>,
    pub bonus: f64,
    #[serde(default)]
    pub activation: f64,
}

pub struct Ability {
    pub name: String,
    pub spells: Vec<i64>,
    pub bonus: f64,
    pub activation: f64,
    pub level: Option<i64>,
}

#[derive(Default)]
pub struct Movement {
    pub abilities: Vec<Ability>,
}

impl Movement {
    pub fn load(conn: &Connection, definitions: &[MovementDef], class: i64) -> Result<Self> {
        let has_trainers: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'm_class_spell')",
            [],
            |r| r.get(0),
        )?;
        let mut abilities = Vec::new();
        for def in definitions {
            let mut spells = def.spells.clone();
            let mut level = None;
            if has_trainers {
                let mut stmt = conn.prepare("SELECT spell_id, level FROM m_class_spell WHERE class_id = ?1 AND name = ?2 ORDER BY level, spell_id")?;
                for row in stmt.query_map(rusqlite::params![class, def.name], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                })? {
                    let (id, at) = row?;
                    spells.push(id);
                    level = Some(level.map_or(at, |old: i64| old.min(at)));
                }
            }
            spells.sort_unstable();
            spells.dedup();
            abilities.push(Ability {
                name: def.name.clone(),
                spells,
                bonus: def.bonus,
                activation: def.activation,
                level,
            });
        }
        Ok(Self { abilities })
    }

    pub fn initial(&self, start: Option<&StartState>, level: i64) -> u64 {
        self.abilities
            .iter()
            .enumerate()
            .filter(|(_, ability)| match start {
                Some(start) => ability.spells.iter().any(|id| start.spells.contains(id)),
                None => ability.level.is_some_and(|at| at <= level),
            })
            .fold(0, |mask, (i, _)| mask | 1 << i)
    }

    pub fn trained(&self, level: i64) -> u64 {
        self.abilities
            .iter()
            .enumerate()
            .filter(|(_, a)| a.level.is_some_and(|at| at <= level))
            .fold(0, |mask, (i, _)| mask | 1 << i)
    }

    /// Optimistic speed for route bounds; activation costs and unsupported terrain can only
    /// make a real trip slower. Abilities and mounts do not stack.
    pub fn speed(&self, known: u64, level: i64, params: &Params) -> f64 {
        self.abilities
            .iter()
            .enumerate()
            .filter(|(i, _)| known & (1 << i) != 0)
            .map(|(_, a)| params.run_speed * (1.0 + a.bonus))
            .fold(params.speed(level), f64::max)
    }

    pub fn walk_time(&self, distance: f64, known: u64, level: i64, params: &Params) -> f64 {
        self.abilities
            .iter()
            .enumerate()
            .filter(|(i, _)| known & (1 << i) != 0)
            .map(|(_, a)| distance / (params.run_speed * (1.0 + a.bonus)) + a.activation)
            .fold(distance / params.speed(level), f64::min)
    }

    pub fn spells(&self) -> impl Iterator<Item = i64> + '_ {
        self.abilities
            .iter()
            .filter(|a| a.level.is_some())
            .flat_map(|a| a.spells.iter().copied())
    }
}
