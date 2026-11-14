//! Export the actual flight and training events used by a profession excursion.

use crate::common::{database_of, repo};
use fg_route::export::{self, StepKind};
use fg_route::job::{self, Overrides, PlanRequest};
use fg_route::plan::{Breakdown, Event, Planner, Timed};
use fg_route::xp::Edition;
use serde_json::json;

#[test]
fn a_traced_training_return_flight_exports_its_departure_and_skip_boundary() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let request: PlanRequest = serde_json::from_value(json!({
        "race":"orc", "class":"warrior", "from_level":27, "to_level":28,
        "required_class_quests":[],
        "params":{"edition":"forever", "profession_trainers":true, "effort":0},
        "professions":[{"key":"fishing", "target":150}],
        "start":{"race":2, "class":"WARRIOR", "level":27,
                 "professions":[{"line":356, "rank":50, "cap":75}]}
    }))
    .unwrap();
    let prepared = job::prepare(&conn, &overrides, &request, &|_| {}).unwrap();
    let nodes = &prepared.world.taxi_nodes;
    let sun_rock = nodes.iter().position(|n| n.id == 29).unwrap();
    let thunder_bluff = nodes.iter().position(|n| n.id == 22).unwrap();
    let trainer = prepared
        .model
        .profession_trainers
        .iter()
        .position(|t| t.loc.id == 3028)
        .unwrap();
    assert_eq!(prepared.model.profession_trainers[trainer].cap, 150);
    let trace: Vec<_> = [
        Event::Fly {
            from: sun_rock,
            to: thunder_bluff,
        },
        Event::ProfTrain {
            profession: 0,
            trainer,
            outbound: 0.0,
            ranks: 0b10,
        },
        Event::Fly {
            from: thunder_bluff,
            to: sun_rock,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(i, event)| Timed {
        event,
        time: (i + 1) as f64 * 60.0,
        level: 27,
        xp: 0,
        power: 27.0,
        gear: 0.0,
        party: vec![],
    })
    .collect();
    let planner = Planner::new(&prepared.model, &prepared.world, &prepared.params, &prepared.profile);
    let route = export::build_traced(&planner, &prepared.profile, &[], &trace, 180.0, Breakdown::default());
    assert_eq!(route.steps.len(), 3);
    assert_eq!(route.steps[0].from_taxi, Some(29));
    assert_eq!(route.steps[0].target.as_ref().unwrap().id, 22);
    assert_eq!(route.steps[1].kind, StepKind::Profession);
    assert_eq!((route.steps[1].line, route.steps[1].skill), (Some(356), Some(150)));
    assert_eq!(route.steps[2].from_taxi, Some(22));
    assert_eq!(route.steps[2].target.as_ref().unwrap().id, 29);
    let saved = serde_json::to_value(&route).unwrap();
    let loaded: fg_route::Route = serde_json::from_value(saved).unwrap();
    let lua = export::to_lua(&[loaded]);
    assert!(lua.contains("pte = 3"));
    assert_eq!(
        lua.matches("pte = ").count(),
        1,
        "only the outbound flight can bypass the excursion"
    );
}
