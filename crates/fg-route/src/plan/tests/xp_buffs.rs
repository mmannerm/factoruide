//! XP options through route replay: acquisition, imported completion, kills and quest rewards.

use super::{planner, quests_at_start};
use crate::job::Prepared;
use crate::model::MateStart;
use crate::plan::{Edit, Kind, Stop};
use crate::xp::{Edition, SLEEPING_BAG_QUEST};

fn prepared() -> Prepared {
    let mut p = quests_at_start(2);
    p.params.edition = Edition::Forever;
    p.profile.from_level = 3;
    p.profile.to_level = 5;
    p.model.quests[0].id = SLEEPING_BAG_QUEST;
    for q in &mut p.model.quests {
        q.level = 3;
        q.xp = 100;
    }
    p.model.index = p.model.quests.iter().enumerate().map(|(i, q)| (q.id, i)).collect();
    p
}

fn stop(index: u32, kind: Kind) -> Edit {
    Edit::Stop {
        stop: Stop::quest(index, kind),
        farm: false,
        spot: None,
    }
}

#[test]
fn food_increases_kills_but_not_quests() {
    let mut p = prepared();
    let edits = [Edit::Grind { mobs: 1 }, stop(1, Kind::Accept), stop(1, Kind::TurnIn)];
    let base = planner(&p).edit(&edits);
    p.params.food_xp = true;
    let report = planner(&p).edit(&edits);
    assert_eq!(report.errors(), 0);
    assert_eq!(report.steps[0].xp, 63);
    assert_eq!(report.steps[2].xp, 100);
    assert_eq!(report.steps[0].end, base.steps[0].end, "no food upkeep time");
}

#[test]
fn sleeping_bag_starts_after_its_reward_and_survives_profile_import() {
    let mut p = prepared();
    p.params.sleeping_bag = true;
    p.params.food_xp = true;
    let report = planner(&p).edit(&[
        Edit::Grind { mobs: 1 },
        stop(0, Kind::Accept),
        stop(0, Kind::TurnIn),
        Edit::Grind { mobs: 1 },
        stop(1, Kind::Accept),
        stop(1, Kind::TurnIn),
    ]);
    assert_eq!(report.errors(), 0);
    assert_eq!(report.steps[0].xp, 63, "no bag before the quest");
    assert_eq!(report.steps[2].xp, 100, "the reward does not boost itself");
    assert_eq!(report.steps[3].xp, 65);
    assert_eq!(report.steps[5].xp, 103);
    assert_eq!(
        report.steps[3].end - report.steps[3].start,
        report.steps[0].end - report.steps[0].start
    );
    p.model.initial.completed = Some([SLEEPING_BAG_QUEST].into());
    let imported = planner(&p).edit(&[Edit::Grind { mobs: 1 }, stop(1, Kind::Accept), stop(1, Kind::TurnIn)]);
    assert_eq!(imported.steps[0].xp, 65);
    assert_eq!(imported.steps[2].xp, 103);
}

#[test]
fn buffed_grinding_reaches_the_same_level_faster_and_consumes_rested_xp() {
    let mut p = prepared();
    p.profile.to_level = 4;
    p.model.initial.rested = 700;
    let (_, base) = planner(&p).simulate_full(&[], None).unwrap();
    let end = planner(&p).end_of(&[]).unwrap();
    p.params.food_xp = true;
    p.params.sleeping_bag = true;
    p.model.initial.completed = Some([SLEEPING_BAG_QUEST].into());
    let (_, boosted) = planner(&p).simulate_full(&[], None).unwrap();
    let buffed_end = planner(&p).end_of(&[]).unwrap();
    assert!((base.grinding / boosted.grinding - 1.08).abs() < 1e-9);
    assert_eq!((end.level, end.xp, end.rested), (4, 0, 0));
    assert_eq!((buffed_end.level, buffed_end.xp, buffed_end.rested), (4, 0, 0));
}

#[test]
fn bag_completion_stays_with_each_member_when_the_lowest_player_changes() {
    let mut p = prepared();
    p.params.sleeping_bag = true;
    p.params.group_size = 2;
    p.model.initial.completed = Some([SLEEPING_BAG_QUEST].into());
    p.model.initial.mates = vec![MateStart {
        level: 3,
        ..MateStart::default()
    }];
    let report = planner(&p).edit(&[Edit::Grind { mobs: 1 }, Edit::Grind { mobs: 1 }]);
    assert_eq!(report.errors(), 0);
    let kills: Vec<_> = report
        .trace
        .iter()
        .filter(|t| matches!(t.event, crate::plan::Event::GrindMobs { .. }))
        .collect();
    assert_eq!(kills[1].party, [(3, 62), (3, 60)]);
}

#[test]
fn forever_options_do_not_change_other_editions() {
    for edition in [Edition::Classic, Edition::Tbc] {
        let mut p = prepared();
        p.params.edition = edition;
        p.params.food_xp = true;
        p.params.sleeping_bag = true;
        p.model.initial.completed = Some([SLEEPING_BAG_QUEST].into());
        let report = planner(&p).edit(&[Edit::Grind { mobs: 1 }, stop(1, Kind::Accept), stop(1, Kind::TurnIn)]);
        assert_eq!(report.steps[0].xp, 60);
        assert_eq!(report.steps[2].xp, 100);
    }
}

#[test]
fn food_also_reduces_the_time_for_a_group_member_to_catch_up() {
    let mut p = super::duo(100);
    p.params.edition = Edition::Forever;
    (p.params.group_sync_window, p.params.group_sync_max) = (0.05, 0.1);
    p.model.initial.mates[0].rested = 50;
    let (_, base) = planner(&p).simulate_full(&super::BOTH, None).unwrap();
    let base_end = planner(&p).end_of(&super::BOTH).unwrap();
    p.params.food_xp = true;
    let (_, boosted) = planner(&p).simulate_full(&super::BOTH, None).unwrap();
    let boosted_end = planner(&p).end_of(&super::BOTH).unwrap();
    assert!(base.catch_up > 0.0);
    assert!((base.catch_up / boosted.catch_up - 1.05).abs() < 1e-9);
    assert_eq!(
        (base_end.level, base_end.xp, base_end.rested),
        (boosted_end.level, boosted_end.xp, boosted_end.rested)
    );
}
