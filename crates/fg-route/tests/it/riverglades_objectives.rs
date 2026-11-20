//! Riverglades interactions retain the game's objective slot and imported progress.

use crate::common::{database_of, repo};
use fg_route::export::StepKind;
use fg_route::job::{self, LogObjective, LogQuest, MapPos, Overrides, PlanRequest, QuestId, StartState};
use fg_route::model::{EntityKind, GuideObjective};
use fg_route::xp::Edition;
use serde_json::json;

fn request(quest: i64, previous: i64, x: f64, y: f64) -> PlanRequest {
    let mut req: PlanRequest = serde_json::from_value(json!({
        "race": "human", "class": "warrior", "from_level": 40, "to_level": 60,
        "required_class_quests": []
    }))
    .unwrap();
    req.objectives = vec![GuideObjective::Quest(quest)];
    req.start = Some(StartState {
        name: "Tester".into(),
        race: 1,
        class: "WARRIOR".into(),
        level: 40,
        position: Some(MapPos { map: 2548, x, y }),
        completed: vec![QuestId(previous)],
        log: vec![LogQuest {
            id: QuestId(quest),
            ..Default::default()
        }],
        ..StartState::default()
    });
    req
}

#[test]
fn marshal_ryan_is_an_interaction_with_the_existing_npc() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let req = request(94887, 94885, 26.16, 40.80);
    let prepared = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    let quest = &prepared.model.quests[prepared.model.index[&94887]];
    assert!(!quest.escort);
    let [talk] = quest.objectives.as_slice() else {
        panic!("one conversation expected")
    };
    assert_eq!(
        (talk.loc.kind, talk.loc.id, talk.slot),
        (EntityKind::Npc, 257250, Some(1))
    );
    assert_eq!((talk.kills, talk.uses, talk.count), (0.0, 0.0, 1.0));

    let outcome = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
    let talk = outcome
        .route
        .steps
        .iter()
        .find(|s| s.quest == Some(94887) && s.kind == StepKind::Objective)
        .expect("Marshal Ryan interaction");
    assert_eq!(talk.slot, Some(1));
    assert_eq!(talk.target.as_ref().map(|t| t.id), Some(257250));
    assert!(talk.text.contains("Talk to Marshal Ryan"));
}

#[test]
fn freshwater_sample_uses_the_provided_phial_and_keeps_completed_progress() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let mut req = request(96084, 96083, 62.0, 56.2);
    let prepared = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    let quest = &prepared.model.quests[prepared.model.index[&96084]];
    let [sample] = quest.objectives.as_slice() else {
        panic!("one phial use expected")
    };
    let expected = prepared.world.to_world(16591, 62.0, 56.2).unwrap();
    assert!(sample.loc.pos.dist(&expected) < 1.0);
    assert_eq!(
        (sample.loc.kind, sample.slot, sample.kills, sample.extra),
        (EntityKind::Area, Some(1), 0.0, 3.0)
    );

    let outcome = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
    let sample = outcome
        .route
        .steps
        .iter()
        .find(|s| s.quest == Some(96084) && s.kind == StepKind::Objective)
        .expect("fill the phial");
    assert_eq!(sample.slot, Some(1));
    assert!(sample.text.contains("Use the Empty Phial"));

    req.start.as_mut().unwrap().log[0].objectives = vec![LogObjective {
        text: "Filled Freshwater Phial: 1/1".into(),
        done: 1,
        need: 1,
        finished: true,
    }];
    let finished = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
    assert!(
        !finished
            .route
            .steps
            .iter()
            .any(|s| s.quest == Some(96084) && s.kind == StepKind::Objective)
    );
    assert!(
        finished
            .route
            .steps
            .iter()
            .any(|s| s.quest == Some(96084) && s.kind == StepKind::TurnIn)
    );
}

#[test]
fn smugglers_paradise_uses_only_the_characters_faction_introduction() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    for (race, race_id, own_intro, other_intro) in [("orc", 2, 93976, 94928), ("human", 1, 94928, 93976)] {
        for completed in [false, true] {
            let mut req = request(95288, own_intro, 77.0, 52.8);
            req.race = race.into();
            let start = req.start.as_mut().unwrap();
            start.race = race_id;
            start.log.clear();
            if !completed {
                start.completed.clear();
            }
            let preview = job::objective_preview(&conn, &overrides, &req).unwrap();
            assert!(
                preview.excluded.is_empty(),
                "{race}, completed={completed}: {:?}",
                preview.excluded
            );
            let outcome = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
            let steps = &outcome.route.steps;
            assert!(!steps.iter().any(|s| s.quest == Some(other_intro)), "{race}");
            let accept = steps
                .iter()
                .position(|s| s.quest == Some(95288) && s.kind == StepKind::Accept)
                .expect("Smuggler's Paradise can be accepted");
            assert!(steps[accept].pre.is_empty());
            assert_eq!(steps[accept].pre_any, [own_intro]);
            let introduction = steps
                .iter()
                .position(|s| s.quest == Some(own_intro) && s.kind == StepKind::TurnIn);
            if completed {
                assert!(introduction.is_none(), "completed introduction must not be repeated");
            } else {
                assert!(
                    introduction.is_some_and(|i| i < accept),
                    "finish the faction's introduction first"
                );
            }
        }
    }
}
