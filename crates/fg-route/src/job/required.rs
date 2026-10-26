//! Resolve must-have quest IDs and names for the character or group.

use super::{PlanRequest, QuestChoice, RequiredQuest};
use crate::model::{CompletionWindow, Profile, QuestRequirement};
use crate::params::Params;
use anyhow::{Result, bail};
use rusqlite::Connection;

pub(super) fn resolve(
    conn: &Connection,
    request: &PlanRequest,
    profile: &Profile,
    params: &Params,
) -> Result<Vec<QuestRequirement>> {
    let mut out: Vec<QuestRequirement> = Vec::new();
    for choice in &request.required_quests {
        let mut stmt = conn.prepare(
            "SELECT id, coalesce(name, ''), coalesce(races_mask, 0), coalesce(classes_mask, 0)
             FROM m_quest WHERE (?1 > 0 AND id = ?1) OR (?2 != '' AND lower(name) = lower(?2) AND status = 'available') ORDER BY id",
        )?;
        let (id, name, window) = match choice {
            RequiredQuest::Id(id) => (*id, "", CompletionWindow::default()),
            RequiredQuest::Name(name) => (0, name.trim(), CompletionWindow::default()),
            RequiredQuest::Window { quest, window } => match quest {
                QuestChoice::Id(id) => (*id, "", *window),
                QuestChoice::Name(name) => (0, name.trim(), *window),
            },
        };
        let label = if id != 0 { id.to_string() } else { name.to_owned() };
        anyhow::ensure!(
            window
                .min_level
                .into_iter()
                .chain(window.max_level)
                .all(|l| (1..=params.edition.rules().max_level).contains(&l)),
            "required quest {label:?}: completion levels must be between 1 and {}",
            params.edition.rules().max_level
        );
        anyhow::ensure!(
            window
                .min_level
                .zip(window.max_level)
                .is_none_or(|(min, max)| min <= max),
            "required quest {label:?}: minimum completion level exceeds maximum"
        );
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
            .filter(|(_, _, races, classes)| profile.members_of(Some(*races), Some(*classes)) != 0)
            .collect();
        let selected = match eligible.as_slice() {
            [quest] => quest.0,
            [] if matches.is_empty() => bail!("required quest {label:?}: not found in this edition's data"),
            [] => bail!("required quest {label:?}: unavailable to this character's race or class"),
            _ => bail!(
                "required quest {label:?}: ambiguous name; choose an ID ({})",
                eligible.iter().map(|q| q.0.to_string()).collect::<Vec<_>>().join(", ")
            ),
        };
        if let Some(max) = window.max_level {
            let members = request.members(params.edition.rules());
            if members.is_empty() {
                anyhow::ensure!(
                    profile.completed_quests.contains(&selected) || profile.from_level <= max,
                    "required quest {label:?}: character is already above completion level {max}"
                );
            } else {
                let (_, _, races, classes) = matches.iter().find(|q| q.0 == selected).unwrap();
                let eligible = profile.members_of(Some(*races), Some(*classes));
                for (_, m) in members.iter().enumerate().filter(|(k, _)| eligible & (1 << k) != 0) {
                    anyhow::ensure!(
                        m.completed.contains(&selected) || m.level <= max,
                        "required quest {label:?}: {} is already above completion level {max}",
                        m.name
                    );
                }
            }
        }
        if let Some(previous) = out.iter_mut().find(|q| q.id == selected) {
            previous.window = CompletionWindow {
                min_level: previous.window.min_level.into_iter().chain(window.min_level).max(),
                max_level: previous.window.max_level.into_iter().chain(window.max_level).min(),
            };
            anyhow::ensure!(
                previous
                    .window
                    .min_level
                    .zip(previous.window.max_level)
                    .is_none_or(|(min, max)| min <= max),
                "required quest {label:?}: completion windows do not overlap"
            );
        } else {
            out.push(QuestRequirement { id: selected, window });
        }
    }
    Ok(out)
}
