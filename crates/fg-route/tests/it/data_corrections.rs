//! Edition corrections through requests, edited guides and generated routes.

use crate::common::{database_of, repo};
use fg_route::job::{self, Editor, Overrides, PlanRequest, RouteStep, StartState};
use fg_route::model::Names;
use fg_route::xp::Edition;
use serde_json::json;

fn request(quests: &serde_json::Value) -> PlanRequest {
    serde_json::from_value(json!({
        "race":"human", "class":"warrior", "from_level":20, "to_level":40,
        "required_class_quests":[], "required_quests":quests
    }))
    .unwrap()
}

fn druid_at_twenty() -> PlanRequest {
    let mut req = request(&json!([]));
    req.race = "skyborne_horde".into();
    req.class = "druid".into();
    req.to_level = 21;
    req.start = Some(StartState {
        level: 20,
        spells: vec![768],
        ..StartState::default()
    });
    req
}

#[test]
fn forever_trains_shred_at_twenty_after_learning_cat_form() {
    let conn = database_of("forever");
    let p = job::prepare(&conn, &overrides(Edition::Forever), &druid_at_twenty(), &|_| {}).unwrap();
    let book = &p.model.abilities;
    let shred = book.bit(5221).unwrap();
    assert_eq!(book.trained(0, 20) & shred, 0, "Cat Form still gates Shred");
    let trained = book.trained(p.model.initial.abilities, 20);
    assert_ne!(trained & shred, 0);
    let power = p.model.power.as_ref().unwrap();
    assert!(power.spell_bonus(20, 20, trained) > power.spell_bonus(20, 20, trained & !shred));
}

#[test]
fn forever_no_longer_plans_training_for_tigers_fury() {
    let conn = database_of("forever");
    let p = job::prepare(&conn, &overrides(Edition::Forever), &druid_at_twenty(), &|_| {}).unwrap();
    let planner = fg_route::plan::Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let guide = fg_route::export::build(&planner, &p.profile, &[]);
    assert!(!guide.spells.iter().any(|id| matches!(id, 5217 | 6793 | 9845 | 9846)));
}

#[test]
fn the_skyborne_wind_bridge_is_open_during_confront_lorthuna() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    let names = Names::load(&conn, "enUS").unwrap();
    for (race, quest) in [("skyborne_horde", 92646), ("skyborne", 93835)] {
        for state in 0..3 {
            let mut req = request(&json!([]));
            req.race = race.into();
            req.from_level = 12;
            req.to_level = 13;
            req.start = Some(StartState {
                level: 12,
                position: Some(job::MapPos {
                    map: 2665,
                    x: 71.614068,
                    y: 50.665884,
                }),
                log: if state == 1 {
                    vec![job::LogQuest {
                        id: quest,
                        complete: true,
                        ..Default::default()
                    }]
                } else {
                    vec![]
                },
                completed: if state == 2 { vec![quest] } else { vec![] },
                ..StartState::default()
            });
            let editor = Editor::open(&conn, &o, &req, None).unwrap();
            let link = editor
                .catalog(&names)
                .links
                .into_iter()
                .find(|l| l.link.name.starts_with("Wind bridge to Rohashi Spires"))
                .unwrap()
                .link;
            let report = editor.simulate(
                &[
                    RouteStep::Link {
                        first: link.clone(),
                        last: link,
                    },
                    RouteStep::TurnIn { quest },
                ],
                &names,
            );
            assert_eq!(
                report.steps[0].status,
                if state == 1 { "done" } else { "error" },
                "{race}, quest state {state}: {:?}",
                report.steps[0]
            );
            if state == 1 {
                assert!(
                    report.steps[0]
                        .line
                        .as_ref()
                        .unwrap()
                        .text
                        .contains("wait for the wind")
                );
            }
        }
    }
}

fn overrides(edition: Edition) -> Overrides {
    let mut o = Overrides::load_edition(&repo().join("overrides"), edition).unwrap();
    o.params.effort = 0.0;
    o.params.candidates = 1;
    o.params.threads = 1;
    o
}

#[test]
fn lightforge_ingots_is_an_alternative_after_the_reports_in_forever_only() {
    for edition in Edition::ALL {
        let conn = database_of(edition.key());
        let o = overrides(edition);
        let mut req = request(&json!([526]));
        req.start = Some(StartState {
            level: 25,
            completed: vec![321],
            ..StartState::default()
        });
        let editor = Editor::open(&conn, &o, &req, None).unwrap();
        let report = editor.simulate(
            &[RouteStep::Accept { quest: 526 }],
            &Names::load(&conn, "enUS").unwrap(),
        );
        assert_eq!(report.errors, usize::from(edition != Edition::Forever));
        if edition == Edition::Forever {
            req.required_quests = serde_json::from_value(json!([324, 526])).unwrap();
            let error = job::prepare(&conn, &o, &req, &|_| {}).err().unwrap();
            assert!(format!("{error:#}").contains("conflicts with quest"));
        }
    }
}

#[test]
fn the_black_shield_waits_for_both_reports() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    for (completed, errors) in [(vec![1273], 1), (vec![1323], 1), (vec![1273, 1323], 0)] {
        let mut req = request(&json!([1276]));
        req.race = "orc".into();
        req.start = Some(StartState {
            level: 35,
            completed,
            ..StartState::default()
        });
        let editor = Editor::open(&conn, &o, &req, None).unwrap();
        let report = editor.simulate(
            &[RouteStep::Accept { quest: 1276 }],
            &Names::load(&conn, "enUS").unwrap(),
        );
        assert_eq!(report.errors, errors);
    }
}

#[test]
fn the_blacklisted_fools_stout_is_not_planned_as_a_child() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    let error = job::prepare(&conn, &o, &request(&json!([1127])), &|_| {})
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("unavailable in this edition"));
    let findings = fg_route::audit::audit(&conn, &o, &["child-quests".into()], &|_| {}).unwrap();
    assert!(!findings.iter().any(|f| f.key == "quest 1119 child 1127"));
    let p = job::prepare(&conn, &o, &request(&json!([1119])), &|_| {}).unwrap();
    assert!(
        p.model.quests[p.model.index[&1119]]
            .objectives
            .iter()
            .all(|o| o.child != Some(1127))
    );
}

#[test]
fn each_forever_druid_can_complete_the_cat_trial_at_twenty_with_the_statues_blessing() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    for (race, reward, trial) in [
        ("skyborne_horde", 98362, 98404),
        ("skyborne", 98397, 98404),
        ("tauren", 98362, 98342),
        ("nightelf", 98397, 98396),
    ] {
        let mut req = request(&json!([{"quest":reward,"min_level":20,"max_level":20}]));
        req.race = race.into();
        req.class = "druid".into();
        req.to_level = 21;
        let guide = job::plan(&conn, &o, &req, &|_| {}).unwrap();
        let objectives: Vec<_> = guide
            .route
            .steps
            .iter()
            .filter(|s| s.quest == Some(trial) && s.kind == fg_route::export::StepKind::Objective)
            .collect();
        let statue = objectives
            .iter()
            .position(|s| s.text.contains("Great Cat Spirit Statue"))
            .unwrap();
        assert!(objectives[statue].text.contains("renew it as needed"));
        let relics: Vec<_> = objectives
            .iter()
            .enumerate()
            .filter(|(_, s)| s.text.contains("Relic of the"))
            .collect();
        assert_eq!(relics.len(), 3, "{race}");
        assert!(
            relics.iter().all(|(at, _)| *at > statue),
            "{race}: blessing precedes the relics"
        );
        assert!(
            guide
                .route
                .steps
                .iter()
                .any(|s| s.quest == Some(reward) && s.kind == fg_route::export::StepKind::TurnIn)
        );
        let editor = Editor::open(&conn, &o, &req, None).unwrap();
        editor.save(&guide.plan, vec![]).unwrap();
        let at: Vec<_> = guide
            .plan
            .iter()
            .enumerate()
            .filter_map(|(at, s)| matches!(s,RouteStep::Objective{quest,..} if *quest==trial).then_some(at))
            .collect();
        let mut reordered = guide.plan.clone();
        reordered.swap(at[0], at[1]);
        let invalid = editor.simulate(&reordered, &Names::load(&conn, "enUS").unwrap());
        assert!(invalid.errors > 0, "{race}: a relic cannot precede the blessing");
        assert!(editor.save(&reordered, vec![]).is_err());
    }
}

#[test]
fn a_new_horde_skyborne_druid_finishes_cat_form_by_twenty_one() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    let mut req = request(&json!([{"quest":98362,"min_level":20,"max_level":21}]));
    req.race = "skyborne_horde".into();
    req.class = "druid".into();
    req.from_level = 1;
    req.to_level = 22;
    req.required_class_quests = None;
    let guide = job::plan(&conn, &o, &req, &|_| {}).unwrap();
    let at = guide
        .plan
        .iter()
        .position(|s| matches!(s, RouteStep::TurnIn { quest: 98362 }))
        .unwrap();
    let report = Editor::open(&conn, &o, &req, None)
        .unwrap()
        .simulate(&guide.plan, &Names::load(&conn, "enUS").unwrap());
    assert_eq!(report.errors, 0);
    assert!((20..=21).contains(&report.steps[at - 1].level));
}
