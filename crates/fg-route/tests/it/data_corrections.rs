//! Edition corrections through requests, edited guides and generated routes.

use crate::common::{database_of, repo};
use fg_route::job::{self, Editor, Overrides, PlanRequest, QuestId, RouteStep, StartState};
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

#[test]
fn glowing_shard_only_enters_wailing_caverns_for_the_pickup() {
    use fg_route::export::StepKind;
    use fg_route::model::GuideObjective;

    for (db, edition) in [
        ("forever", Edition::Forever),
        ("classic", Edition::Classic),
        ("tbc", Edition::Tbc),
    ] {
        let conn = database_of(db);
        let overrides = overrides(edition);
        let mut req = request(&json!([]));
        req.race = "orc".into();
        req.objectives = vec![GuideObjective::Quest(6981)];
        req.start = Some(StartState {
            name: "Tester".into(),
            level: 26,
            position: Some(job::MapPos {
                map: 1413,
                x: 62.98,
                y: 37.22,
            }),
            ..StartState::default()
        });
        for (accepted, talked) in [(true, false), (false, false), (true, true)] {
            req.start.as_mut().unwrap().log = if accepted {
                vec![job::LogQuest {
                    id: QuestId(6981),
                    complete: talked,
                    objectives: vec![],
                }]
            } else {
                vec![]
            };
            let outcome = job::plan(&conn, &overrides, &req, &|_| {}).unwrap();
            let steps = &outcome.route.steps;
            let pickup = |step: &&fg_route::export::Step| step.kind == StepKind::Accept && step.quest == Some(6981);
            let dungeons = steps.iter().filter(|s| s.kind == StepKind::Dungeon).count();
            assert_eq!(dungeons, usize::from(!accepted), "{db}, accepted={accepted}");
            assert_eq!(steps.iter().filter(pickup).count(), usize::from(!accepted), "{db}");
            if !accepted {
                let start = steps.iter().find(pickup).unwrap();
                assert!(start.inside && start.mob_ids.contains(&3654), "{db}");
            }
            let talk = steps
                .iter()
                .find(|s| s.kind == StepKind::Objective && s.quest == Some(6981));
            assert_eq!(talk.is_some(), !talked, "{db}");
            if let Some(talk) = talk {
                assert!(
                    !talk.inside
                        && talk.zone_id == Some(17)
                        && talk.target.as_ref().is_some_and(|p| p.id == 3442)
                        && talk.text.contains("Talk to Sputtervalve"),
                    "{db}"
                );
            }
            assert!(
                steps.iter().any(|s| s.kind == StepKind::TurnIn
                    && s.quest == Some(6981)
                    && s.target.as_ref().is_some_and(|p| p.id == 8418)),
                "{db}"
            );
        }
    }
}

#[test]
fn a_group_needs_the_shard_pickup_until_all_its_members_have_the_quest() {
    let conn = database_of("forever");
    let mut overrides = overrides(Edition::Forever);
    overrides.params.dungeons = false;
    let mut req = request(&json!([6981]));
    req.race = "orc".into();
    req.to_level = 27;
    let leader = StartState {
        name: "Leader".into(),
        race: 2,
        class: "WARRIOR".into(),
        faction: Some(fg_route::faction::Faction::Horde),
        level: 26,
        log: vec![job::LogQuest {
            id: QuestId(6981),
            complete: true,
            objectives: vec![],
        }],
        ..StartState::default()
    };
    req.start = Some(leader.clone());
    req.mates = vec![StartState {
        name: "Partner".into(),
        log: vec![],
        ..leader
    }];

    for (partner_has_quest, needs_run) in [(false, true), (true, false)] {
        if partner_has_quest {
            req.mates[0].log = vec![job::LogQuest {
                id: QuestId(6981),
                ..Default::default()
            }];
        }
        let prepared = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
        let wc = prepared.model.dungeons.iter().find(|d| d.area == 718).unwrap();
        assert_eq!(wc.needed, needs_run, "partner has quest = {partner_has_quest}");
    }
}

#[test]
fn group_shard_steps_follow_the_members_who_still_need_it() {
    use fg_route::export::StepKind;
    use fg_route::model::GuideObjective;

    let conn = database_of("forever");
    let mut req = request(&json!([]));
    req.race = "orc".into();
    let leader = StartState {
        name: "Leader".into(),
        race: 2,
        class: "WARRIOR".into(),
        faction: Some(fg_route::faction::Faction::Horde),
        level: 26,
        // Every other Wailing Caverns quest in the frozen Forever snapshot is already done.
        completed: [914, 959, 962, 999, 1486, 1487, 1489, 1490, 1491, 1500, 3366]
            .into_iter()
            .map(QuestId)
            .collect(),
        position: Some(job::MapPos {
            map: 1413,
            x: 62.98,
            y: 37.22,
        }),
        ..StartState::default()
    };
    for objective in [GuideObjective::Quest(6981), GuideObjective::Dungeon(718)] {
        req.objectives = vec![objective];
        for (leader_turned_in, partner_accepted, partner_ready) in [
            (true, true, false),
            (true, false, false),
            (false, true, false),
            (true, true, true),
        ] {
            let mut start = leader.clone();
            if leader_turned_in {
                start.completed.push(QuestId(6981));
            } else {
                start.log = vec![job::LogQuest {
                    id: QuestId(6981),
                    complete: true,
                    objectives: vec![],
                }];
            }
            req.start = Some(start);
            req.mates = vec![StartState {
                name: "Partner".into(),
                log: if partner_accepted {
                    vec![job::LogQuest {
                        id: QuestId(6981),
                        complete: partner_ready,
                        objectives: vec![],
                    }]
                } else {
                    vec![]
                },
                ..leader.clone()
            }];
            let outcome = job::plan(&conn, &overrides(Edition::Forever), &req, &|_| {}).unwrap();
            let steps = &outcome.route.steps;
            let label = format!(
                "{objective:?}: leader turned in={leader_turned_in}, partner accepted={partner_accepted}, ready={partner_ready}"
            );
            assert_eq!(
                steps.iter().filter(|s| s.kind == StepKind::Dungeon).count(),
                usize::from(!partner_accepted),
                "{label}"
            );
            assert_eq!(
                steps
                    .iter()
                    .filter(|s| s.kind == StepKind::Accept && s.quest == Some(6981))
                    .count(),
                usize::from(!partner_accepted),
                "{label}"
            );
            assert_eq!(
                steps
                    .iter()
                    .filter(|s| s.kind == StepKind::Objective
                        && s.quest == Some(6981)
                        && s.target.as_ref().is_some_and(|p| p.id == 3442)
                        && !s.inside)
                    .count(),
                usize::from(!partner_ready),
                "{label}"
            );
            assert!(
                steps
                    .iter()
                    .any(|s| s.kind == StepKind::TurnIn && s.quest == Some(6981)),
                "{label}"
            );
        }
    }
}

#[test]
fn mothers_milk_still_requires_blackrock_spire() {
    use fg_route::model::GuideObjective;

    let conn = database_of("forever");
    let mut req = request(&json!([]));
    req.objectives = vec![GuideObjective::Quest(4866)];
    req.start = Some(StartState {
        level: 60,
        ..StartState::default()
    });
    let prepared = job::prepare(&conn, &overrides(Edition::Forever), &req, &|_| {}).unwrap();
    let spire = prepared.model.dungeons.iter().find(|d| d.area == 1583).unwrap();
    assert!(spire.needed);
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
                        id: QuestId(quest),
                        complete: true,
                        ..Default::default()
                    }]
                } else {
                    vec![]
                },
                completed: if state == 2 { vec![QuestId(quest)] } else { vec![] },
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
fn lightforge_ingots_follows_the_lost_ingots_in_forever_only() {
    for edition in Edition::ALL {
        let conn = database_of(edition.key());
        let o = overrides(edition);
        let names = Names::load(&conn, "enUS").unwrap();
        let mut req = request(&json!([526]));
        req.start = Some(StartState {
            level: 25,
            completed: vec![QuestId(321)],
            ..StartState::default()
        });
        let editor = Editor::open(&conn, &o, &req, None).unwrap();
        let report = editor.simulate(&[RouteStep::Accept { quest: 526 }], &names);
        assert_eq!(report.errors, 1, "{}: before The Lost Ingots", edition.key());

        req.start.as_mut().unwrap().completed.push(QuestId(324));
        if edition == Edition::Forever {
            let editor = Editor::open(&conn, &o, &req, None).unwrap();
            let report = editor.simulate(&[RouteStep::Accept { quest: 526 }], &names);
            assert_eq!(report.errors, 0, "after The Lost Ingots");
        } else {
            let error = job::prepare(&conn, &o, &req, &|_| {}).err().unwrap();
            assert!(format!("{error:#}").contains("conflicts with quest 324"));
        }
    }
}

#[test]
fn forever_can_require_both_lost_ingots_and_lightforge_ingots() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    let mut req = request(&json!([324, 526]));
    req.start = Some(StartState {
        level: 25,
        completed: vec![QuestId(321)],
        ..StartState::default()
    });
    let p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
    for id in [324, 526] {
        assert!(p.model.quests[p.model.index[&id]].required, "quest {id}");
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
            completed: completed.into_iter().map(QuestId).collect(),
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
fn the_blacklisted_fools_stout_cannot_be_required() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    let error = job::prepare(&conn, &o, &request(&json!([1127])), &|_| {})
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("missing, unavailable, or restricted to another race/class"));
    let p = job::prepare(&conn, &o, &request(&json!([1119])), &|_| {}).unwrap();
    assert!(p.model.quests[p.model.index[&1119]].required);
    assert!(!p.model.index.contains_key(&1127));
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

#[test]
fn trial_blessings_preserve_the_original_relic_objective_indices() {
    let conn = database_of("forever");
    let mut old = overrides(Edition::Forever);
    old.quest_overrides.iter_mut().for_each(|q| q.stealth_object = None);
    let mut req = druid_at_twenty();
    req.start = None;
    let before = job::prepare(&conn, &old, &req, &|_| {}).unwrap();
    let after = job::prepare(&conn, &overrides(Edition::Forever), &req, &|_| {}).unwrap();
    let original = &before.model.quests[before.model.index[&98404]].objectives;
    let current = &after.model.quests[after.model.index[&98404]].objectives;
    assert_eq!(current.len(), original.len() + 1);
    for (a, b) in original.iter().zip(current) {
        assert_eq!((a.loc.kind, a.loc.id, &a.text), (b.loc.kind, b.loc.id, &b.text));
    }
    assert_eq!(current.last().unwrap().loc.id, 673394);
}

#[test]
fn data_audits_reject_unknown_override_quests_and_prerequisites() {
    for edition in Edition::ALL {
        let conn = database_of(edition.key());
        let mut baseline = overrides(edition);
        // Only the graph audit is needed here, not a model for every race.
        baseline.races.clear();
        let checks = ["exclusive-prerequisites".to_owned()];
        fg_route::audit::audit(&conn, &baseline, &checks, &|_| {}).unwrap();
        for (id, pre) in [(21, 999_999_999), (999_999_999, 783)] {
            let mut o = baseline.clone();
            o.quest_overrides
                .push(serde_json::from_value(json!({"id":id,"pre_all":[pre]})).unwrap());
            let error = fg_route::audit::audit(&conn, &o, &checks, &|_| {}).err().unwrap();
            assert!(format!("{error:#}").contains("999999999"), "{error:#}");
        }
    }
}

#[test]
fn present_quest_corrections_reject_unknown_prerequisites() {
    for edition in Edition::ALL {
        let conn = database_of(edition.key());
        let mut o = overrides(edition);
        o.quest_overrides
            .push(serde_json::from_value(json!({"id":21,"pre_all":[999_999_999]})).unwrap());
        let error = job::prepare(&conn, &o, &request(&json!([])), &|_| {}).err().unwrap();
        assert!(
            format!("{error:#}").contains("references unknown quest 999999999"),
            "{error:#}"
        );
    }
}

#[test]
fn forever_call_of_fire_keeps_braziers_as_interactions() {
    let conn = database_of("forever");
    let o = overrides(Edition::Forever);
    let mut req = request(&json!([97257]));
    req.race = "skyborne_horde".into();
    req.class = "shaman".into();
    req.from_level = 10;
    req.to_level = 60;
    req.start = Some(StartState {
        level: 10,
        completed: vec![QuestId(97245)],
        ..StartState::default()
    });
    let p = job::prepare(&conn, &o, &req, &|_| {}).unwrap();
    let q = &p.model.quests[p.model.index[&97257]];
    assert_eq!(q.time_limit, Some(360.0));
    assert_eq!(q.objectives.len(), 2);
    for (o, (npc, slot)) in q.objectives.iter().zip([(268762, 1), (268679, 2)]) {
        assert_eq!((o.loc.id, o.kills, o.uses, o.slot), (npc, 0.0, 1.0, Some(slot)));
    }
    assert!(q.objectives[0].text.contains("ritual with Olariaan"));
    assert!(q.objectives[1].text.contains("Torch of Eternal Flame"));
    let lightning = &p.model.quests[p.model.index[&92550]];
    assert!(
        lightning
            .objectives
            .iter()
            .any(|o| o.loc.id == 251662 && (o.kills - 4.0).abs() < f64::EPSILON),
        "neutral combat objectives remain kills"
    );
    let mut stale = o;
    stale
        .quest_overrides
        .iter_mut()
        .find(|q| q.id == 97257)
        .unwrap()
        .interactions
        .as_mut()
        .unwrap()[0]
        .npc = 999_999_999;
    let error = job::prepare(&conn, &stale, &req, &|_| {}).err().unwrap();
    assert!(
        format!("{error:#}").contains("interaction target is not an NPC objective"),
        "{error:#}"
    );
}
