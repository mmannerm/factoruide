//! Mage event locations, preparation order, timers and editor persistence.
use crate::common::{database_of, repo};
use fg_route::export::StepKind;
use fg_route::job::{self, Editor, LogObjective, LogQuest, MapPos, Overrides, PlanRequest, QuestId, StartState};
use fg_route::model::{GuideObjective, Names};
use fg_route::xp::Edition;
use serde_json::json;

fn request(level: i64, quest: i64, previous: i64, position: MapPos) -> PlanRequest {
    let mut req: PlanRequest = serde_json::from_value(json!({
        "race":"orc", "class":"mage", "from_level":level, "to_level":60,
        "required_class_quests":[]
    }))
    .unwrap();
    req.objectives = vec![GuideObjective::Quest(quest)];
    req.start = Some(StartState {
        name: "Tester".into(),
        race: 2,
        class: "MAGE".into(),
        level,
        completed: vec![QuestId(previous)],
        position: Some(position),
        ..StartState::default()
    });
    req
}

#[test]
fn horde_coffers_stay_in_undercity_after_both_tools_and_editor_save() {
    let conn = database_of("forever");
    let ov = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let mut req = request(
        16,
        1960,
        1959,
        MapPos {
            map: 1458,
            x: 66.49,
            y: 50.04,
        },
    );
    req.start.as_mut().unwrap().log = vec![LogQuest {
        id: QuestId(1960),
        ..Default::default()
    }];
    let out = job::plan(&conn, &ov, &req, &|_| {}).unwrap();
    let editor = Editor::open(&conn, &ov, &req, None).unwrap();
    let plan = editor.steps(&out).0;
    assert_eq!(editor.simulate(&plan, &Names::new("enUS")).errors, 0);
    let saved = editor.save(&plan, out.notes.clone()).unwrap();
    for route in [&out.route, &saved.route] {
        let steps: Vec<_> = route
            .steps
            .iter()
            .filter(|s| s.kind == StepKind::Objective && s.quest == Some(1960))
            .collect();
        assert_eq!(steps.len(), 3);
        assert!(steps.iter().all(|s| s.zone_id == Some(1497)));
        assert_eq!(steps.last().unwrap().slot, Some(1));
        assert_eq!(steps.last().unwrap().target.as_ref().unwrap().id, 7292);
        assert!(steps.last().unwrap().text.contains("subdue"));
        assert!(
            !route
                .steps
                .iter()
                .any(|s| s.zone_id == Some(1519) || s.kind == StepKind::Link)
        );
        assert!(
            route
                .steps
                .iter()
                .any(|s| s.quest == Some(1960) && s.kind == StepKind::TurnIn)
        );
    }
    // Saved-log progress still refers to the original game slots, not the execution order.
    req.start.as_mut().unwrap().log[0].objectives = vec![
        LogObjective {
            text: "Filled Containment Coffer: 0/3".into(),
            done: 0,
            need: 3,
            finished: false,
        },
        LogObjective {
            text: "Chest of Containment Coffers: 1/1".into(),
            done: 1,
            need: 1,
            finished: true,
        },
        LogObjective {
            text: "Cantation of Manifestation: 1/1".into(),
            done: 1,
            need: 1,
            finished: true,
        },
    ];
    let resumed = job::plan(&conn, &ov, &req, &|_| {}).unwrap();
    let remaining: Vec<_> = resumed
        .route
        .steps
        .iter()
        .filter(|s| s.quest == Some(1960) && s.kind == StepKind::Objective)
        .collect();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].slot, Some(1));
}

#[test]
fn alliance_shared_coffers_keep_stormwind_sources() {
    let conn = database_of("forever");
    let ov = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let mut req = request(
        16,
        1920,
        1919,
        MapPos {
            map: 1453,
            x: 50.0,
            y: 87.0,
        },
    );
    req.race = "human".into();
    let start = req.start.as_mut().unwrap();
    start.race = 1;
    start.log = vec![LogQuest {
        id: QuestId(1920),
        ..Default::default()
    }];
    let prepared = job::prepare(&conn, &ov, &req, &|_| {}).unwrap();
    let quest = &prepared.model.quests[prepared.model.index[&1920]];
    let coffer = quest.objectives.iter().find(|o| o.loc.id == 7292).unwrap();
    assert_eq!(coffer.loc.zone, 1519);
    assert!(coffer.alts.iter().all(|o| o.loc.zone != 1497));
}

#[test]
fn mana_surges_keeps_combat_and_finishes_before_timer_through_editor() {
    let conn = database_of("forever");
    let ov = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let req = request(
        43,
        1958,
        1956,
        MapPos {
            map: 1445,
            x: 46.0,
            y: 57.0,
        },
    );
    let prepared = job::prepare(&conn, &ov, &req, &|_| {}).unwrap();
    let quest = &prepared.model.quests[prepared.model.index[&1957]];
    assert_eq!(quest.time_limit, Some(600.0));
    let event = &quest.objectives[0];
    assert_eq!((event.kills, event.count), (12.0, 12.0));
    assert!(event.mob_level >= 35.0);
    assert!(!event.mobs.is_empty());
    assert_eq!(event.pool.wait, 0.0);
    let mut ordinary = ov.clone();
    ordinary.quest_overrides.retain(|q| q.id != 1957);
    let prior = job::prepare(&conn, &ordinary, &req, &|_| {}).unwrap();
    assert!(prior.model.quests[prior.model.index[&1957]].objectives[0].pool.wait > 600.0);

    let out = job::plan(&conn, &ov, &req, &|_| {}).unwrap();
    let editor = Editor::open(&conn, &ov, &req, None).unwrap();
    let plan = editor.steps(&out).0;
    assert_eq!(editor.simulate(&plan, &Names::new("enUS")).errors, 0);
    let mut delayed = plan.clone();
    let start = delayed
        .iter()
        .position(|s| matches!(s, job::RouteStep::Accept { quest: 1957 }))
        .unwrap();
    delayed.insert(start + 1, job::RouteStep::Grind { mobs: 100 });
    assert!(
        editor.simulate(&delayed, &Names::new("enUS")).errors > 0,
        "the timer must still reject a late event"
    );
    let saved = editor.save(&plan, out.notes.clone()).unwrap();
    for route in [&out.route, &saved.route] {
        let accept = route
            .steps
            .iter()
            .find(|s| s.quest == Some(1957) && s.kind == StepKind::Accept)
            .unwrap();
        let turnin = route
            .steps
            .iter()
            .find(|s| s.quest == Some(1957) && s.kind == StepKind::TurnIn)
            .unwrap();
        assert!(turnin.time - accept.time > 0.0 && turnin.time - accept.time <= 600.0);
        assert!(
            route
                .steps
                .iter()
                .any(|s| s.quest == Some(1958) && s.kind == StepKind::TurnIn)
        );
    }
}
