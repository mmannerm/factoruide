//! Published object positions restore Riverglades quests missing from the bundled map data.

use crate::common::{database_of, repo};
use fg_route::export::StepKind;
use fg_route::job::{self, LogQuest, MapPos, Overrides, PlanRequest, QuestId, StartState};
use fg_route::model::GuideObjective;
use fg_route::xp::Edition;
use serde_json::json;

#[test]
fn riverglades_object_locations_restore_gathering_and_item_use_routes() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let mut ids: Vec<_> = overrides.object_locations.iter().map(|o| o.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, [620422, 620424, 621767, 629425, 629426, 629427, 642517]);
    for id in &ids {
        let exists: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM m_object WHERE id = ?1)", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(exists, "location correction refers to missing object {id}");
    }
    for edition in [Edition::Classic, Edition::Tbc] {
        assert!(
            Overrides::load_edition(&repo().join("overrides"), edition)
                .unwrap()
                .object_locations
                .is_empty()
        );
    }
    for (quest, race, race_id, targets) in [
        (94734, "orc", 2, vec![620424, 620422]),
        (94843, "human", 1, vec![621767]),
        (95615, "human", 1, vec![642517]),
        (95165, "human", 1, vec![629425, 629426, 629427]),
    ] {
        let mut req: PlanRequest = serde_json::from_value(json!({
            "race": race, "class": "warrior", "from_level": 44, "to_level": 60,
            "required_class_quests": []
        }))
        .unwrap();
        req.objectives = vec![GuideObjective::Quest(quest)];
        req.start = Some(StartState {
            name: "Tester".into(),
            race: race_id,
            class: "WARRIOR".into(),
            level: 44,
            position: Some(MapPos {
                map: 2548,
                x: 60.0,
                y: 60.0,
            }),
            log: vec![LogQuest {
                id: QuestId(quest),
                ..Default::default()
            }],
            ..StartState::default()
        });
        let outcome = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
        for target in targets {
            assert!(
                outcome.route.steps.iter().any(|s| s.quest == Some(quest)
                    && s.kind == StepKind::Objective
                    && s.target.as_ref().is_some_and(|t| t.id == target)
                    && s.zone_id == Some(16591)),
                "quest {quest} must visit object {target} in Riverglades"
            );
        }
        assert!(
            outcome
                .route
                .steps
                .iter()
                .any(|s| s.quest == Some(quest) && s.kind == StepKind::TurnIn)
        );
    }
}
