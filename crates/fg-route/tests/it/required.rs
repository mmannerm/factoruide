//! Explicit quest requirements through request resolution, model construction and guide saving.

use crate::common::{database, database_of, repo};
use fg_route::job::{self, Overrides, PlanRequest, RequiredQuest, RouteStep, StartState};
use fg_route::plan::{Kind, Planner, Stop};
use fg_route::xp::Edition;

fn request(race: &str, class: &str, required: &[RequiredQuest]) -> PlanRequest {
    serde_json::from_value(serde_json::json!({
        "race": race, "class": class, "from_level": 1, "to_level": 5,
        "required_class_quests": [], "required_quests": required,
    }))
    .unwrap()
}

#[test]
fn required_quest_chain_is_included_and_survives_zero_penalty() {
    let conn = database();
    let mut overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    overrides.params.mandatory_penalty = 0.0;
    let req = request("human", "warrior", &[RequiredQuest::Id(21)]); // Skirmish at Echo Ridge
    let p = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    let target = &p.model.quests[p.model.index[&21]];
    assert!(target.required);
    assert_ne!(target.pre_any, [] as [i64; 0]);
    for pre in &target.pre_any {
        assert!(p.model.quests[p.model.index[pre]].required);
    }
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let route = planner.greedy();
    planner.validate_required(&route).unwrap();
    assert!(planner.end_of(&route).unwrap().turned.contains(&p.model.index[&21]));
    assert!(planner.validate_required(&[]).is_err());
}

#[test]
fn required_quest_editor_refuses_removal() {
    let conn = database();
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    let req = request("human", "warrior", &[RequiredQuest::Id(783)]);
    let editor = job::Editor::open(&conn, &overrides, &req, None).unwrap();
    assert!(editor.save(&[], vec![]).is_err());
    editor
        .save(
            &[RouteStep::Accept { quest: 783 }, RouteStep::TurnIn { quest: 783 }],
            vec![],
        )
        .unwrap();
}

#[test]
fn required_quest_completed_profile_needs_no_repeat() {
    let conn = database();
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    let mut req = request("human", "warrior", &[RequiredQuest::Id(783)]);
    req.start = Some(StartState {
        level: 2,
        completed: vec![783],
        ..StartState::default()
    });
    let p = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    assert!(p.model.quests.iter().all(|q| !q.required));
}

#[test]
fn required_quest_resolution_rejects_unknown_restricted_and_excluded() {
    let conn = database();
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    for (choice, reason) in [
        (RequiredQuest::Id(999_999_999), "not found"),
        (RequiredQuest::Id(3087), "race or class"),
    ] {
        let req = request("human", "warrior", &[choice]);
        let error = job::prepare(&conn, &overrides, &req, &|_| {}).err().unwrap();
        assert!(format!("{error:#}").contains(reason), "{error:#}");
    }
    let mut req = request("human", "warrior", &[RequiredQuest::Id(21)]);
    let mut params = overrides.params.clone();
    params.excluded_quests.push(21);
    req.params = Some(params);
    let error = job::prepare(&conn, &overrides, &req, &|_| {}).err().unwrap();
    assert!(format!("{error:#}").contains("excluded by configuration"), "{error:#}");
}

#[test]
fn required_sleeping_bag_chain_for_horde_skyborne_druid() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let mut req = request(
        "skyborne_horde",
        "druid",
        &[RequiredQuest::Name("This Must Be The Place".into())],
    );
    req.to_level = 30;
    let p = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    for id in [79007, 79192, 79975, 79976] {
        assert!(p.model.quests[p.model.index[&id]].required, "quest {id}");
    }
    assert!(
        !p.model.quests.iter().any(|q| q.required && q.id == 79008),
        "Alliance starter must not be required"
    );
}

#[test]
fn required_quests_continue_after_target_level() {
    let conn = database();
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    let req = request("human", "warrior", &[RequiredQuest::Id(783)]);
    let mut p = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    let i = p.model.index[&783];
    p.model.quests[i].xp = 100_000;
    // A different completed quest reaches the level target before the required quest.
    let other = p.model.index[&7];
    p.model.quests[other].objectives.clear();
    p.model.quests[other].pre_all.clear();
    p.model.quests[other].pre_any.clear();
    p.model.quests[other].xp = 100_000;
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let route = [
        Stop::quest(other as u32, Kind::Accept),
        Stop::quest(other as u32, Kind::TurnIn),
        Stop::quest(i as u32, Kind::Accept),
        Stop::quest(i as u32, Kind::TurnIn),
    ];
    planner.validate_required(&route).unwrap();
}

#[test]
fn required_sleeping_bag_route_can_be_completed_and_exported() {
    let conn = database_of("forever");
    let mut overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    overrides.params.dungeons = false;
    let mut req = request("skyborne_horde", "druid", &[RequiredQuest::Id(79976)]);
    req.from_level = 30;
    req.to_level = 31;
    let p = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let route = planner.greedy();
    planner.validate_required(&route).unwrap();
    let guide = fg_route::export::build(&planner, &p.profile, &route);
    assert!(
        guide
            .steps
            .iter()
            .any(|s| s.quest == Some(79976) && s.kind == fg_route::export::StepKind::TurnIn)
    );
}

#[test]
fn required_quest_name_ambiguity_lists_ids() {
    let conn = database();
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    let name: String = conn
        .query_row(
            "SELECT name FROM m_quest WHERE status = 'available'
        AND (coalesce(races_mask,0) = 0 OR (races_mask & 1) != 0)
        AND (coalesce(classes_mask,0) = 0 OR (classes_mask & 1) != 0)
        GROUP BY lower(name) HAVING count(*) > 1 ORDER BY name LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let req = request("human", "warrior", &[RequiredQuest::Name(name)]);
    let error = job::prepare(&conn, &overrides, &req, &|_| {}).err().unwrap();
    assert!(format!("{error:#}").contains("ambiguous name; choose an ID"));
}

#[test]
fn required_quest_completed_by_one_group_member_is_still_required_by_the_other() {
    let conn = database();
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    let mut req = request("human", "warrior", &[RequiredQuest::Id(783)]);
    let leader = StartState {
        name: "Leader".into(),
        race: 1,
        class: "WARRIOR".into(),
        faction: "Alliance".into(),
        level: 10,
        completed: vec![783],
        ..StartState::default()
    };
    req.start = Some(leader.clone());
    req.mates = vec![StartState {
        name: "Partner".into(),
        completed: vec![],
        ..leader
    }];
    req.to_level = 11;
    let p = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    assert!(planner.validate_required(&[]).is_err());
    let i = p.model.index[&783] as u32;
    planner
        .validate_required(&[Stop::quest(i, Kind::Accept), Stop::quest(i, Kind::TurnIn)])
        .unwrap();
}
