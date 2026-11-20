//! Verified Riverglades item starters through selection, planning and imported progress.

use crate::common::{database_of, repo};
use fg_route::export::StepKind;
use fg_route::job::{self, LogQuest, Overrides, PlanRequest, QuestId, RequiredQuest, StartState};
use fg_route::model::{EntityKind, GuideObjective};
use fg_route::xp::Edition;
use serde_json::json;

fn request(race: &str, race_id: i64, quest: i64) -> PlanRequest {
    serde_json::from_value(json!({
        "race": race, "class": "warrior", "from_level": 1, "to_level": 60,
        "required_class_quests": [], "objectives": [{"quest": quest}],
        "start": {
            "name": "Riverglades", "race": race_id, "class": "WARRIOR", "level": 60,
            "position": {"map": 2548, "x": 60.0, "y": 47.0}
        }
    }))
    .unwrap()
}

#[test]
fn item_started_quests_are_selectable_and_acquire_each_starter_once() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    for (race, race_id, quest, item, mob) in [
        ("orc", 2, 93971, 263930, 257079),
        ("human", 1, 95935, 271612, 263141),
        ("orc", 2, 95936, 271613, 263141),
    ] {
        let choices = job::quest_choices(&conn, &overrides, race, "warrior", &quest.to_string(), None).unwrap();
        assert_eq!(choices.iter().map(|q| q.id).collect::<Vec<_>>(), [quest]);
        let mut req = request(race, race_id, quest);
        let prepared = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
        let q = &prepared.model.quests[prepared.model.index[&quest]];
        assert!(q.starts.iter().any(|l| l.kind == EntityKind::Item && l.id == item));
        assert_eq!(q.start_mob.as_ref().map(|m| m.0), Some(mob));
        assert!(q.start_kills > 0.0);
        assert!(q.objectives.is_empty(), "starter must not be farmed twice: {quest}");
        assert!(
            job::objective_preview(&conn, &overrides, &req)
                .unwrap()
                .excluded
                .is_empty()
        );

        let outcome = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
        let pickup: Vec<_> = outcome
            .route
            .steps
            .iter()
            .filter(|s| s.kind == StepKind::Accept && s.quest == Some(quest))
            .collect();
        assert_eq!(pickup.len(), 1);
        assert_eq!(pickup[0].target.as_ref().map(|t| t.id), Some(item));
        assert!(pickup[0].mob_ids.contains(&mob));
        assert!(
            !outcome
                .route
                .steps
                .iter()
                .any(|s| s.kind == StepKind::Objective && s.quest == Some(quest))
        );

        req.start.as_mut().unwrap().log = vec![LogQuest {
            id: QuestId(quest),
            complete: true,
            objectives: vec![],
        }];
        let completed = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
        assert!(
            !completed
                .route
                .steps
                .iter()
                .any(|s| s.quest == Some(quest) && matches!(s.kind, StepKind::Accept | StepKind::Objective))
        );
        assert!(
            completed
                .route
                .steps
                .iter()
                .any(|s| s.kind == StepKind::TurnIn && s.quest == Some(quest))
        );
    }
}

#[test]
fn verified_availability_keeps_factions_and_unrelated_removed_quests_out() {
    let conn = database_of("forever");
    let mut overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    // A correction to another field must not revive a removed quest.
    overrides
        .quest_overrides
        .push(serde_json::from_value(json!({"id": 96027, "pre_all": []})).unwrap());
    let start = StartState {
        race: 2,
        class: "WARRIOR".into(),
        ..StartState::default()
    };
    let with_starters = job::objective_counts(&conn, &overrides, &start).unwrap();
    let mut without_availability = overrides.clone();
    for q in &mut without_availability.quest_overrides {
        q.available = None;
    }
    let without_starters = job::objective_counts(&conn, &without_availability, &start).unwrap();
    assert_eq!(with_starters.zones[&16591], without_starters.zones[&16591] + 2);
    for (race, denied) in [("human", [93971, 95936]), ("orc", [95935, 96027])] {
        for quest in denied {
            assert!(
                job::quest_choices(&conn, &overrides, race, "warrior", &quest.to_string(), None)
                    .unwrap()
                    .is_empty()
            );
        }
    }
    let mut req = request("orc", 2, 93971);
    req.objectives.clear();
    req.required_quests = vec![RequiredQuest::Name("Farholde Scouting Report".into())];
    req.start = None;
    req.to_level = 60;
    let prepared = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    assert!(prepared.model.quests[prepared.model.index[&93971]].required);

    // A faction override is not permission to revive a removed quest in another edition.
    for edition in [Edition::Classic, Edition::Tbc] {
        let other = Overrides::load_edition(&repo().join("overrides"), edition).unwrap();
        assert!(
            other
                .quest_overrides
                .iter()
                .all(|q| q.start_item.is_none() && q.available.is_none())
        );
    }
}

#[test]
fn farholde_starting_report_retains_its_level_38_requirement() {
    let conn = database_of("forever");
    let mut overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    overrides.params.power_curve = vec![(1, 100.0), (60, 100.0)];
    let mut req = request("orc", 2, 93971);
    req.objectives = vec![GuideObjective::Quest(93971)];
    req.start = Some(StartState {
        race: 2,
        class: "WARRIOR".into(),
        level: 38,
        ..req.start.unwrap()
    });
    let prepared = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    assert_eq!(prepared.model.quests[prepared.model.index[&93971]].min_level, 38);
}
