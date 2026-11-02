//! Item-quest acquisition through the real edition data, simulation and guide export.

use crate::common::{database_of, repo};
use fg_route::job::{self, Overrides, PlanRequest};
use fg_route::plan::{Kind, Planner, Stop};
use fg_route::xp::Edition;
use serde_json::json;

fn request(race: &str, quest: i64) -> PlanRequest {
    serde_json::from_value(json!({
        "race":race, "class":"warrior", "from_level":10, "to_level":11,
        "required_class_quests":[], "required_quests":[quest],
        "start":{"level":10,"position":{"map":if race == "human" {1453} else {1454},"x":50.0,"y":50.0}}
    }))
    .unwrap()
}

#[test]
fn shipping_label_buys_and_fills_a_crate_before_the_faction_turn_in() {
    let conn = database_of("forever");
    let mut o = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    o.params.crowded = 1.0;
    o.params.crowd_by_level = vec![1.0];
    for (race, id, label, ender, level) in [
        ("skyborne_horde", 98248, 280180, 256386, 10),
        // Stormwind's existing unknown-faction ambient spawns gate its AH at level 25.
        ("human", 98247, 280179, 256390, 25),
    ] {
        let mut req = request(race, id);
        req.from_level = level;
        req.to_level = level + 1;
        req.start.as_mut().unwrap().level = level;
        let mut p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
        let i = p.model.index[&id];
        let q = &p.model.quests[i];
        assert_eq!(q.held, None, "the label starts its quest before reaching the camp");
        assert_eq!(q.starts[0].id, label);
        assert_eq!(q.min_level, 10);
        assert_eq!(q.ends[0].id, ender);
        assert!(
            q.objectives.is_empty(),
            "the label comes from filling, not invented mob kills"
        );
        let purchase = q.auction_start.as_ref().unwrap();
        assert_eq!(
            purchase.items.iter().map(|i| (i.id, i.count)).collect::<Vec<_>>(),
            [(248682, 1), (2447, 20)]
        );
        let route = [Stop::quest(i as u32, Kind::Accept), Stop::quest(i as u32, Kind::TurnIn)];
        let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
        planner.validate_required(&route).unwrap();
        let guide = fg_route::export::build(&planner, &p.profile, &route);
        assert!(guide.total_time.is_finite());
        let accept = guide
            .steps
            .iter()
            .find(|s| s.quest == Some(id) && s.kind == fg_route::export::StepKind::Accept)
            .unwrap();
        assert!(accept.text.contains("auction house"));
        assert!(accept.text.contains("Peacebloom"));
        assert!(accept.text.contains("20"));
        assert!(accept.text.contains("Use Shipping Label to start Shipping Label"));
        assert_eq!(accept.level, level, "{race}: {:?}, guard {:?}", q.starts, q.start_guard);
        eprintln!(
            "{race} auction: {:?}; guard {:?}; accept level {}",
            q.starts, q.start_guard, accept.level
        );
        assert!(accept.text.contains("fill the Apprentice Herbs crate"));
        assert!(
            !accept.held,
            "the addon must wait for accepting the quest after filling"
        );
        assert!(
            guide
                .steps
                .iter()
                .any(|s| s.quest == Some(id) && s.kind == fg_route::export::StepKind::TurnIn)
        );
        let turnin = guide
            .steps
            .iter()
            .find(|s| s.quest == Some(id) && s.kind == fg_route::export::StepKind::TurnIn)
            .unwrap();
        assert_eq!(turnin.say[0].key, "turnin");
        let pack = fg_route::export::polestar::pack(&[("shipping".into(), guide.clone())], "test");
        let lua = &pack.iter().find(|(file, _)| *file == "Guides.lua").unwrap().1;
        assert_eq!(lua.matches(format!("t = \"accept\", quest = {id}").as_str()).count(), 1);
        // Compare the same route without purchase work: no travel or kill-XP difference.
        p.model.quests[i].auction_start = None;
        let plain_planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
        let base = fg_route::export::build(&plain_planner, &p.profile, &route);
        assert!((guide.total_time - base.total_time - (120.0 + p.params.object_time)).abs() < 0.01);
        assert_eq!(guide.breakdown.mob_xp, base.breakdown.mob_xp);
    }
}

#[test]
fn shipping_label_does_not_buy_unsolicited_crates_or_repeat_completed_introductions() {
    let conn = database_of("forever");
    let o = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let mut req = request("skyborne_horde", 98248);
    req.required_quests.clear();
    let p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
    assert!(!p.model.index.contains_key(&98248));
    req.required_quests = serde_json::from_value(json!([98248])).unwrap();
    req.start.as_mut().unwrap().completed = vec![job::QuestId(98248)];
    let p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    planner.validate_required(&[]).unwrap();
    let guide = fg_route::export::build(&planner, &p.profile, &[]);
    assert!(!guide.steps.iter().any(|s| s.quest == Some(98248)));
    req.start.as_mut().unwrap().completed.clear();
    req.start.as_mut().unwrap().log = vec![job::LogQuest {
        id: job::QuestId(98248),
        complete: true,
        objectives: vec![],
    }];
    let p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
    let i = p.model.index[&98248];
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let route = [Stop::quest(i as u32, Kind::TurnIn)];
    planner.validate_required(&route).unwrap();
    let guide = fg_route::export::build(&planner, &p.profile, &route);
    assert!(
        guide
            .steps
            .iter()
            .all(|s| s.say.iter().all(|p| p.key != "objective.buy.auction"))
    );
}

#[test]
fn shipping_label_waits_until_level_ten() {
    let conn = database_of("forever");
    let o = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let mut req = request("skyborne_horde", 98248);
    req.from_level = 9;
    req.to_level = 10;
    req.start.as_mut().unwrap().level = 9;
    let p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
    let i = p.model.index[&98248];
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let route = [Stop::quest(i as u32, Kind::Accept), Stop::quest(i as u32, Kind::TurnIn)];
    let guide = fg_route::export::build(&planner, &p.profile, &route);
    let accept = guide
        .steps
        .iter()
        .find(|s| s.quest == Some(98248) && s.kind == fg_route::export::StepKind::Accept)
        .unwrap();
    assert!(accept.level >= 10);
    assert_eq!(accept.min_level, Some(10));
}

#[test]
fn shipping_label_still_needs_an_accessible_auction_house() {
    let conn = database_of("forever");
    let mut o = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    o.params.excluded_zones = conn.prepare("SELECT DISTINCT s.zone_id FROM m_spawn s JOIN m_npc n ON s.entity_type='npc' AND s.entity_id=n.id WHERE coalesce(n.npc_flags,0) & 4096 != 0").unwrap()
        .query_map([], |row| row.get(0)).unwrap().collect::<Result<Vec<i64>,_>>().unwrap();
    let mut req = request("skyborne_horde", 98248);
    let error = job::prepare(&conn, &o, &req, &|_| {}).err().unwrap();
    assert!(format!("{error:#}").contains("no reachable quest giver"));
    req.start.as_mut().unwrap().log = vec![job::LogQuest {
        id: job::QuestId(98248),
        complete: true,
        objectives: vec![],
    }];
    let p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
    let i = p.model.index[&98248];
    assert!(p.model.quests[i].auction_start.is_none());
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let route = [Stop::quest(i as u32, Kind::TurnIn)];
    planner.validate_required(&route).unwrap();
    assert!(
        fg_route::export::build(&planner, &p.profile, &route)
            .total_time
            .is_finite()
    );
    req.start.as_mut().unwrap().log[0].complete = false;
    let error = job::prepare(&conn, &o, &req, &|_| {}).err().unwrap();
    assert!(format!("{error:#}").contains("active item quest is not ready to turn in"));
    // One ready player's label cannot supply the introduction for a new group member.
    let first = req.start.as_mut().unwrap();
    first.log[0].complete = true;
    first.name = "First".into();
    first.race = 96;
    first.class = "WARRIOR".into();
    first.faction = Some(fg_route::faction::Faction::Horde);
    let mut second = first.clone();
    second.name = "Second".into();
    second.log.clear();
    req.mates = vec![second];
    let error = job::prepare(&conn, &o, &req, &|_| {}).err().unwrap();
    assert!(format!("{error:#}").contains("no reachable quest giver"), "{error:#}");
}
