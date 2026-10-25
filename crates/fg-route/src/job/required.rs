//! Resolve explicit quest choices without interpreting a name as a class-quest preference.

use super::{PlanRequest, RequiredQuest};
use anyhow::{Result, bail};
use rusqlite::Connection;

pub(super) fn resolve(conn: &Connection, request: &PlanRequest, race: i64, class: i64) -> Result<Vec<i64>> {
    let race = crate::model::race_bit(race);
    let class = 1 << (class - 1);
    let mut out = Vec::new();
    for choice in &request.required_quests {
        let mut stmt = conn.prepare(
            "SELECT id, coalesce(name, ''), coalesce(races_mask, 0), coalesce(classes_mask, 0)
             FROM m_quest WHERE (?1 > 0 AND id = ?1) OR (?2 != '' AND lower(name) = lower(?2) AND status = 'available') ORDER BY id",
        )?;
        let (id, name) = match choice {
            RequiredQuest::Id(id) => (*id, ""),
            RequiredQuest::Name(name) => (0, name.trim()),
        };
        let matches = stmt
            .query_map(rusqlite::params![id, name], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let eligible: Vec<_> = matches
            .iter()
            .filter(|(_, _, races, classes)| {
                (*races == 0 || races & race != 0) && (*classes == 0 || classes & class != 0)
            })
            .collect();
        let label = match choice {
            RequiredQuest::Id(id) => id.to_string(),
            RequiredQuest::Name(name) => name.clone(),
        };
        let selected = match eligible.as_slice() {
            [quest] => quest.0,
            [] if matches.is_empty() => bail!("required quest {label:?}: not found in this edition's data"),
            [] => bail!("required quest {label:?}: unavailable to this character's race or class"),
            _ => bail!(
                "required quest {label:?}: ambiguous name; choose an ID ({})",
                eligible.iter().map(|q| q.0.to_string()).collect::<Vec<_>>().join(", ")
            ),
        };
        if !out.contains(&selected) {
            out.push(selected);
        }
    }
    Ok(out)
}
