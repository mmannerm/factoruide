//! Small routes built on the Classic database: the incremental evaluation of a changed route
//! gives the cost of its full replay, objectives started and done together.

use super::{Kind, Planner, Stop};
use crate::job::{self, Overrides, PlanRequest, Prepared};
use crate::model::{Guard, Initial, Objective};
use crate::xp::Edition;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn database_of(edition: &str) -> Connection {
    static UNPACK: Mutex<()> = Mutex::new(());
    let _guard = UNPACK.lock().unwrap();
    let (path, packed) = (
        repo().join(format!("data/{edition}.sqlite")),
        repo().join(format!("data/{edition}.sqlite.zst")),
    );
    let modified = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if !path.exists() || modified(&packed) > modified(&path) {
        let data = zstd::decode_all(std::fs::File::open(&packed).unwrap()).unwrap();
        std::fs::write(&path, data).unwrap();
    }
    Connection::open(path).unwrap()
}

fn database() -> Connection {
    database_of("classic")
}

/// Level 1 human warrior with only `n` quests of its own: taken and turned in at the start NPC,
/// nothing to do, no XP (each test sets what it checks). Nothing done on the way.
pub(crate) fn quests_at_start(n: usize) -> Prepared {
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    let request = PlanRequest {
        race: "human".into(),
        class: "warrior".into(),
        from_level: 1,
        to_level: 2,
        name: None,
        params: Some(overrides.params.clone()),
        required_class_quests: None,
        required_quests: vec![],
        professions: vec![],
        locale: None,
        mates: vec![],
        start: None,
    };
    at_start(job::prepare(&database(), &overrides, &request, &|_| {}).unwrap(), n)
}

fn at_start(mut p: Prepared, n: usize) -> Prepared {
    let here = p.model.start.clone();
    let template = p.model.quests[0].clone();
    p.model.quests = (0..n)
        .map(|i| {
            let mut q = template.clone();
            (q.id, q.name, q.level, q.min_level, q.xp, q.xp_observed) =
                (900_000 + i as i64, format!("quest {i}"), 1, 1, 0, true);
            (q.starts, q.ends) = (vec![here.clone()], vec![here.clone()]);
            q.objectives.clear();
            (q.pre_all, q.pre_any, q.exclusive, q.breadcrumb_for) = (vec![], vec![], vec![], None);
            (q.start_kills, q.start_uses, q.mandatory, q.skill, q.power) = (0.0, 0.0, false, None, None);
            (q.start_guard, q.end_guard) = (Guard::default(), Guard::default());
            (q.reputation, q.zone_forced) = (vec![], false);
            q
        })
        .collect();
    p.model.index = p.model.quests.iter().enumerate().map(|(i, q)| (q.id, i)).collect();
    (p.model.chain_top, p.model.unlocks, p.model.rewards) = (vec![1; n], vec![0.0; n], vec![(vec![], vec![]); n]);
    p.model.initial = Initial {
        abilities: p.model.initial.abilities,
        ..Initial::default()
    };
    p.model.power = None;
    p.model.trainers.clear();
    p.model.teleports.clear();
    p.model.vendors.clear();
    p.model.explore.clear();
    p.model.explore_by_zone.clear();
    (p.params.camp_radius, p.params.farm_on_way, p.params.flight_learn_radius) = (0.0, false, 0.0);
    p
}

fn planner(p: &Prepared) -> Planner<'_> {
    Planner::new(&p.model, &p.world, &p.params, &p.profile)
}

#[test]
fn movement_training_changes_travel_and_exports_the_spell_for_each_class() {
    let conn = database();
    for (edition, race, class, spell, seconds) in [
        (Edition::Classic, "nightelf", "druid", 783, 61.22449),
        (Edition::Classic, "orc", "shaman", 2645, 63.22449),
        (Edition::Classic, "orc", "hunter", 5118, 65.93407),
        (Edition::Forever, "orc", "shaman", 2645, 61.22449),
        (Edition::Tbc, "orc", "shaman", 2645, 61.22449),
        (Edition::Forever, "orc", "hunter", 5118, 65.93407),
    ] {
        let overrides = Overrides::load_edition(&repo().join("overrides"), edition).unwrap();
        let req: PlanRequest = serde_json::from_value(serde_json::json!({
            "race":race, "class":class, "from_level":30, "to_level":31,
            "required_class_quests":[], "start":{"level":30, "spells":[]}
        }))
        .unwrap();
        let mut p = at_start(job::prepare(&conn, &overrides, &req, &|_| {}).unwrap(), 1);
        let a = crate::world::Pos {
            continent: 1,
            x: -100.0,
            y: -100.0,
            zone: 0,
            z: None,
        };
        p.model.initial.pos = Some((a, 0));
        let mut trainer = p.model.start.clone();
        trainer.pos = a;
        p.model.trainers = vec![trainer];
        (p.params.run_speed, p.params.detour) = (7.0, 1.0);
        p.params.mount_level = 40; // Compare the spells before buying a mount, also in TBC.
        let quest = [
            super::Edit::Stop {
                stop: Stop::quest(0, Kind::Accept),
                farm: false,
                spot: None,
            },
            super::Edit::Stop {
                stop: Stop::quest(0, Kind::TurnIn),
                farm: false,
                spot: None,
            },
        ];
        let trained = [super::Edit::Train { trainer: 0 }, quest[0], quest[1]];
        // Two 300-yard legs; Ghost Wolf pays its cast time on each. Seven-yard legs can
        // favor walking, and a level-40 mount is faster than any of these abilities.
        for (level, distance, mixed, expected) in [
            (30, 300.0, false, Some(seconds)),
            (
                30,
                7.0,
                false,
                Some(if class == "shaman" && edition == Edition::Classic {
                    2.0
                } else if class == "hunter" {
                    1.53846
                } else {
                    1.42857
                }),
            ),
            (40, 300.0, false, None),
            (30, 300.0, true, None),
        ] {
            p.profile.from_level = level;
            p.profile.to_level = level + 1;
            p.world.terrain = crate::terrain::movement_test_ground(mixed);
            let q = &mut p.model.quests[0];
            q.level = level;
            q.xp = 100_000;
            q.starts[0].pos = crate::world::Pos { x: a.x - distance, ..a };
            q.ends[0].pos = a;
            let planner = planner(&p).explicit();
            let before = planner.edit(&quest);
            let after = planner.edit(&trained);
            assert_eq!(after.errors(), 0);
            if let Some(seconds) = expected {
                assert!(
                    (after.breakdown.travel - seconds).abs() < 0.05,
                    "{class}: two outdoor legs, got {}",
                    after.breakdown.travel
                );
                assert!(after.breakdown.travel <= before.breakdown.travel);
                assert_eq!(
                    planner.edit(&quest).breakdown.travel,
                    before.breakdown.travel,
                    "untrained replay keeps its speed"
                );
                if distance > 7.0 {
                    let automatic = Planner::new(&p.model, &p.world, &p.params, &p.profile);
                    let guide = crate::export::build(&automatic, &p.profile, &automatic.greedy());
                    assert!(
                        guide
                            .steps
                            .iter()
                            .any(|step| step.kind == crate::export::StepKind::Train && step.spells.contains(&spell)),
                        "{class}: generated guides schedule movement training"
                    );
                    assert!(
                        guide.spells.contains(&spell),
                        "{class}: the addon purchase list includes the movement spell"
                    );
                }
            } else {
                assert_eq!(
                    after.breakdown.travel, before.breakdown.travel,
                    "{class}: short legs, mounts and mixed terrain retain their baseline"
                );
            }
        }
        // Recorded spells and synthetic mid-level assumptions both apply from the first leg.
        let mut req = req;
        req.start.as_mut().unwrap().spells = vec![spell];
        for start in [req.start.clone(), None] {
            req.start = start;
            let known = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
            p.model.initial.abilities = known.model.initial.abilities;
            p.world.terrain = crate::terrain::movement_test_ground(false);
            let edited = planner(&p).explicit().edit(&quest);
            assert!(
                (edited.breakdown.travel - seconds).abs() < 0.05,
                "{class}: known spell applies from the first leg"
            );
        }
        p.params.assume_movement_talents = false;
        let without = planner(&p).explicit().edit(&quest);
        let expected = match (class, edition) {
            ("shaman", Edition::Tbc) => 65.22449,
            ("shaman", _) => 67.22449,
            _ => seconds,
        };
        assert!(
            (without.breakdown.travel - expected).abs() < 0.05,
            "the talent option changes Ghost Wolf's cast time, not Cheetah or Travel Form"
        );
    }
}

/// Swapping two accepts leaves the same state after them but for a timed quest's deadline: the
/// rest of the route is replayed (the quest is turned in in time now, it expired before).
#[test]
fn timed_quest_deadline_is_part_of_the_state() {
    let mut p = quests_at_start(2);
    p.model.quests[0].time_limit = Some(400.0);
    p.model.quests[0].xp = 400;
    // Taking quest 1 takes time.
    p.model.quests[1].start_uses = 100.0;
    let planner = planner(&p);
    let route = [
        Stop::quest(0, Kind::Accept),
        Stop::quest(1, Kind::Accept),
        Stop::quest(1, Kind::TurnIn),
        Stop::quest(0, Kind::TurnIn),
    ];
    let swapped = [route[1], route[0], route[2], route[3]];
    let run = planner.run(&route).unwrap();
    let incremental = planner.evaluate(&run, &swapped, 0, 1, true).unwrap();
    let full = planner.simulate(&swapped, None).unwrap();
    assert!(
        (incremental - full).abs() < 1e-6,
        "incremental {incremental} != full {full}"
    );
}

/// An objective done at another of its places, where a quest of the route is taken and turned
/// in at hand: removing that quest replays the objective's stop, whose place is the one really
/// visited (not the objective's nearest place).
#[test]
fn quests_at_hand_are_looked_for_where_the_stop_went() {
    let mut p = quests_at_start(2);
    p.params.camp_radius = 10.0;
    p.world.terrain = crate::terrain::Terrain::default();
    let (mut near, mut far) = (p.model.start.clone(), p.model.start.clone());
    near.pos.x += 100.0;
    far.pos.x += 300.0;
    // The nearest place takes 20 minutes more than the other one.
    let mut objective = Objective {
        loc: near,
        text: "Gather object".into(),
        kills: 0.0,
        mob_level: 0,
        uses: 1.0,
        extra: 1200.0,
        dungeon: None,
        elite: false,
        count: 1.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 1,
        pack: 1.0,
        spots: None,
        guard: Guard::default(),
        beside_giver: false,
        item: false,
        alts: vec![],
        after: 0,
        handed_by: vec![],
        child: None,
        child_name: None,
        vendors: vec![],
        auction: vec![],
        slot: None,
        stock: None,
    };
    let mut other = objective.clone();
    (other.loc, other.extra) = (far.clone(), 0.0);
    objective.alts = vec![other];
    p.model.quests[0].objectives = vec![objective];
    (p.model.quests[1].starts, p.model.quests[1].ends) = (vec![far.clone()], vec![far]);
    p.model.quests[1].xp = 100;
    let planner = planner(&p);
    let route = [
        Stop::quest(0, Kind::Accept),
        Stop::quest(0, Kind::Objective(0)),
        Stop::quest(1, Kind::Accept),
        Stop::quest(1, Kind::TurnIn),
        Stop::quest(0, Kind::TurnIn),
    ];
    let without = [route[0], route[1], route[4]];
    let run = planner.run(&route).unwrap();
    let incremental = planner.evaluate(&run, &without, 2, route.len(), false).unwrap();
    let full = planner.simulate(&without, None).unwrap();
    assert!(
        (incremental - full).abs() < 1e-6,
        "incremental {incremental} != full {full}"
    );
}

/// An objective started before the route (a character met in game) whose places need different
/// kills (drop rates): its share done counts at the other place. 10 items, 10 % done where it
/// takes 20 kills, leave 9 kills where it takes 10.
#[test]
fn objective_started_counts_its_share_at_another_place() {
    let mut p = quests_at_start(1);
    p.profile.to_level = 10;
    p.world.terrain = crate::terrain::Terrain::default();
    let mut loc = p.model.start.clone();
    loc.pos.x += 100.0;
    let mut objective = Objective {
        loc: loc.clone(),
        text: "Gather 10 items".into(),
        kills: 20.0,
        mob_level: 1,
        uses: 0.0,
        extra: 0.0,
        dungeon: None,
        elite: false,
        count: 10.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 20,
        pack: 1.0,
        spots: None,
        guard: Guard::default(),
        beside_giver: false,
        item: false,
        alts: vec![],
        after: 0,
        handed_by: vec![],
        child: None,
        child_name: None,
        vendors: vec![],
        auction: vec![],
        slot: None,
        stock: None,
    };
    let mut other = objective.clone();
    (other.loc.pos.x, other.kills) = (loc.pos.x + 200.0, 10.0);
    objective.alts = vec![other];
    p.model.quests[0].objectives = vec![objective];
    p.model.initial.accepted = vec![0];
    p.model.initial.progress = vec![(0, 0, 0.1)];
    let planner = planner(&p);
    let mut s = planner.initial_state();
    s.pos.x += 300.0;
    let xp = s.spent.mob_xp;
    planner.apply(&mut s, Stop::quest(0, Kind::Objective(0)), None);
    let each = planner.combat().mob_xp(1, &p.model.quests[0].objectives[0].alts[0]);
    let kills = (s.spent.mob_xp - xp) as f64 / each;
    assert!((kills - 9.0).abs() < 1e-6, "{kills} kills at the other place");
    assert_eq!(s.objectives[0], 1);
}

/// Objects gathered right after the kills of another quest at the same place take part of the
/// kills' time (`gather_overlap`); with another stop between them, nothing is saved: each stop
/// stands on its own, what is done together comes from the route's order.
#[test]
fn gathering_next_to_kills_saves_time_when_consecutive() {
    let p = camp_of_two_objectives();
    let planner = planner(&p);
    let fighting = |route: &[Stop]| planner.simulate_full(route, None).unwrap().1.fighting;
    let (kill, gather) = (Stop::quest(0, Kind::Objective(0)), Stop::quest(1, Kind::Objective(0)));
    let (k, g) = (fighting(&[kill]), fighting(&[gather]));
    let saved = g.min(k * p.params.gather_overlap);
    assert!(saved > 1.0);
    assert!((fighting(&[kill, gather]) - (k + g - saved)).abs() < 1e-6);
    assert!((fighting(&[gather, kill]) - (k + g - saved)).abs() < 1e-6);
    // Another stop between them (a quest taken at the start).
    let apart = fighting(&[kill, Stop::quest(2, Kind::Accept), gather]);
    assert!((apart - (k + g)).abs() < 1e-6, "{apart} with a stop between");
}

/// Kill and gathering objectives at a camp 100 yards east of the start, for quests 0 and 1 (in
/// the log), and a third quest given at the start.
fn camp_of_two_objectives() -> Prepared {
    let mut p = quests_at_start(3);
    p.profile.to_level = 10;
    p.world.terrain = crate::terrain::Terrain::default();
    let mut camp = p.model.start.clone();
    camp.pos.x += 100.0;
    let kills = Objective {
        loc: camp,
        text: "Kill 10 mobs".into(),
        kills: 10.0,
        mob_level: 1,
        uses: 0.0,
        extra: 0.0,
        dungeon: None,
        elite: false,
        count: 10.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 10,
        pack: 1.0,
        spots: None,
        guard: Guard::default(),
        beside_giver: false,
        item: false,
        alts: vec![],
        after: 0,
        handed_by: vec![],
        child: None,
        child_name: None,
        vendors: vec![],
        auction: vec![],
        slot: None,
        stock: None,
    };
    let mut gather = Objective {
        text: "Gather 8 objects".into(),
        kills: 0.0,
        uses: 8.0,
        count: 8.0,
        ..kills.clone()
    };
    gather.loc.pos.x += 50.0;
    p.model.quests[0].objectives = vec![kills];
    p.model.quests[1].objectives = vec![gather];
    p.model.initial.accepted = vec![0, 1];
    p
}

/// The last pass brings objectives done at the same place next to each other (the guide then
/// shows them together): the gathering among the kills saves its share.
#[test]
fn objectives_at_one_place_are_brought_together() {
    let p = camp_of_two_objectives();
    let planner = planner(&p);
    let (kill, gather) = (Stop::quest(0, Kind::Objective(0)), Stop::quest(1, Kind::Objective(0)));
    let accept = Stop::quest(2, Kind::Accept);
    let apart = vec![kill, accept, gather];
    let together = planner.route_together(apart.clone());
    assert_eq!(together, vec![kill, gather, accept]);
    let fighting = |route: &[Stop]| planner.simulate_full(route, None).unwrap().1.fighting;
    assert!(fighting(&together) < fighting(&apart) - 1.0);
}

/// A move that would make the route worse is not kept.
#[test]
fn bringing_together_never_makes_it_worse() {
    let p = camp_of_two_objectives();
    let planner = planner(&p);
    let (kill, gather) = (Stop::quest(0, Kind::Objective(0)), Stop::quest(1, Kind::Objective(0)));
    let apart = vec![kill, Stop::quest(2, Kind::Accept), gather];
    let places = vec![
        Some(p.model.quests[0].objectives[0].loc.pos),
        None,
        Some(p.model.quests[1].objectives[0].loc.pos),
    ];
    // Any order but this one scores worse.
    let kept = planner.bring_together(
        apart.clone(),
        |s| Some(*s),
        places,
        |r| Some(if r == apart.as_slice() { 0.0 } else { 1.0 }),
    );
    assert_eq!(kept, apart);
}

/// A quest the route takes and gives up when it goes gray costs `abandon_weight`: the same route
/// without it is better by that much (its taking costs only seconds).
#[test]
fn a_quest_taken_and_given_up_costs_the_route() {
    let mut p = quests_at_start(2);
    p.profile.to_level = 20;
    // Quest 1 takes the character to level 7: quest 0 (level 1, leading nowhere) is gray then.
    p.model.quests[1].xp = 11_200;
    let route = [
        Stop::quest(0, Kind::Accept),
        Stop::quest(1, Kind::Accept),
        Stop::quest(1, Kind::TurnIn),
    ];
    let without = [route[1], route[2]];
    let score = |p: &Prepared, route: &[Stop]| planner(p).simulate_full(route, None).unwrap();
    p.params.abandon_weight = 600.0;
    let ((given_up, spent), (left_out, _)) = (score(&p, &route), score(&p, &without));
    assert_eq!(spent.abandoned, 1);
    assert!(given_up - left_out > 590.0, "{given_up} vs {left_out}");
    p.params.abandon_weight = 0.0;
    let (given_up, _) = score(&p, &route);
    assert!(given_up - left_out < 30.0, "{given_up} vs {left_out}");
}

/// A profession quest (Camping 101: its reward is the trainer's recipe, not XP) taken early and
/// handed in once gray is not given up, and leaving it out costs `profession_quest_weight`, once
/// for a group of exclusive alternatives (the same quest at another race's trainer).
#[test]
fn profession_quests_are_kept_once_gray() {
    let mut p = quests_at_start(3);
    p.profile.to_level = 20;
    // Quest 1 takes the character to level 7: quest 0 (level 1, leading nowhere) is gray then.
    p.model.quests[1].xp = 11_200;
    let mut skinning = crate::model::ProfessionPlan::new(&crate::profession::ProfessionGoal {
        key: "skinning".into(),
        target: 300,
        start_level: None,
        milestones: vec![],
    })
    .unwrap();
    skinning.curve = vec![1; 61];
    p.model.professions = vec![skinning];
    let (camping, other) = (p.model.quests[0].id, p.model.quests[2].id);
    p.model.quests[0].skill = Some((0, 1));
    p.model.quests[2].skill = Some((0, 1));
    p.model.quests[0].exclusive = vec![other];
    p.model.quests[2].exclusive = vec![camping];
    p.params.profession_quest_weight = 300.0;
    let route = [
        Stop::quest(0, Kind::Accept),
        Stop::quest(1, Kind::Accept),
        Stop::quest(1, Kind::TurnIn),
        Stop::quest(0, Kind::TurnIn),
    ];
    let without = [route[1], route[2]];
    let score = |p: &Prepared, route: &[Stop]| planner(p).simulate_full(route, None).unwrap();
    let ((kept, spent), (left_out, spent_without)) = (score(&p, &route), score(&p, &without));
    assert_eq!(spent.abandoned, 0, "not given up once gray");
    assert_eq!(spent.missing_profession_quests, 0);
    assert_eq!(spent_without.missing_profession_quests, 1, "one for both alternatives");
    assert!(left_out - kept > 270.0, "{kept} vs {left_out}");
    // Without the profession leveled, the quest is nothing the guide misses.
    p.model.quests[0].skill = None;
    p.model.quests[2].skill = None;
    assert_eq!(score(&p, &without).1.missing_profession_quests, 0);
}

/// A road east of the start lined with mobs of level 1: three of quest 0's (its objective is
/// done at the end of the road) and three others. Farming on the way is on, without a cap, and
/// every mob met is killed (`farm_share` 1).
fn road_of_mobs() -> (Prepared, crate::world::Pos, crate::world::Pos) {
    let mut p = quests_at_start(1);
    p.profile.to_level = 10;
    p.world.terrain = crate::terrain::Terrain::default();
    (
        p.params.farm_on_way,
        p.params.farm_cap,
        p.params.farm_share,
        p.params.crowded,
    ) = (true, 1.0, 1.0, 0.0);
    let start = p.model.start.pos;
    let mut end = p.model.start.clone();
    end.pos.x += 400.0;
    let mut spawns = Vec::new();
    for i in 0..3 {
        let x = start.x + 100.0 * f64::from(i + 1);
        spawns.push((crate::world::Pos { x, ..start }, 1, 777));
        spawns.push((
            crate::world::Pos {
                x,
                y: start.y + 5.0,
                ..start
            },
            1,
            888,
        ));
    }
    p.model.quests[0].objectives = vec![Objective {
        loc: end.clone(),
        text: "Kill 3 wolves".into(),
        kills: 3.0,
        mob_level: 1,
        uses: 0.0,
        extra: 0.0,
        dungeon: None,
        elite: false,
        count: 3.0,
        mobs: vec![(777, "Wolf".into())],
        pull: 1.0,
        spawns: 3,
        pack: 1.0,
        spots: None,
        guard: Guard::default(),
        beside_giver: false,
        item: false,
        alts: vec![],
        after: 0,
        handed_by: vec![],
        child: None,
        child_name: None,
        vendors: vec![],
        auction: vec![],
        slot: None,
        stock: None,
    }];
    p.model.farm = crate::model::FarmMobs::from_spawns(&spawns, &p.model.quests);
    (p, start, end.pos)
}

/// Mobs killed and XP farmed on the walk from `a` to `b`, from state `s`.
fn farmed_on_walk(
    planner: &Planner<'_>,
    mut s: super::State,
    a: &crate::world::Pos,
    b: &crate::world::Pos,
) -> (u32, i64) {
    let mut kills = 0;
    planner.farm_along(&mut s, a, b, &mut |_, e| {
        if let super::Event::Farm { kills: k, .. } = e {
            kills += k;
        }
    });
    (kills, s.spent.farm_xp)
}

/// The mobs of a quest of the log are not farmed on the way, to its objective or back: the
/// objective counts them where it is done (GitHub #20). Before it is taken, and once it is
/// turned in, they are mobs like the others.
#[test]
fn the_logs_quest_mobs_are_not_farmed_on_the_way() {
    let (p, start, end) = road_of_mobs();
    let planner = planner(&p);
    let s0 = planner.initial_state();
    assert_eq!(farmed_on_walk(&planner, s0.clone(), &start, &end).0, 6, "not taken yet");
    let mut taken = s0.clone();
    taken.accepted.set(0, true);
    assert_eq!(
        farmed_on_walk(&planner, taken.clone(), &start, &end).0,
        3,
        "on the way there"
    );
    let mut done = taken.clone();
    done.objectives[0] = 1;
    assert_eq!(
        farmed_on_walk(&planner, done.clone(), &end, &start).0,
        3,
        "on the way back"
    );
    let mut turned = done;
    turned.turned.set(0, true);
    assert_eq!(farmed_on_walk(&planner, turned, &start, &end).0, 6, "turned in");
}

/// Farming on the way kills `farm_share` of the mobs met, and fewer on a crowded server: the
/// share left by the other players around, as at an objective's spot (one in 1 + crowd ×
/// `crowd_players`).
#[test]
fn farming_on_the_way_kills_a_share_of_the_mobs_met() {
    let (mut p, start, end) = road_of_mobs();
    let xp = |p: &Prepared| {
        let planner = planner(p);
        farmed_on_walk(&planner, planner.initial_state(), &start, &end).1 as f64
    };
    let all = xp(&p);
    assert!(all > 0.0);
    p.params.farm_share = 0.8;
    let most = xp(&p);
    assert!((most - 0.8 * all).abs() <= 1.0, "{most} of {all}");
    (p.params.crowded, p.params.crowd_players) = (1.0, 3.0);
    let crowd = p.params.crowd_at(1);
    assert!(crowd > 0.0);
    let crowded = xp(&p);
    let expected = 0.8 * all / (1.0 + crowd * 3.0);
    assert!((crowded - expected).abs() <= 1.0, "{crowded} vs {expected}");
}

/// A quest given at the start whose mobs are a few levels above the character: nothing else to
/// do, the greedy construction grinds to the level they need and does it, rather than stopping.
#[test]
fn greedy_grinds_to_the_mobs_of_a_quest_in_reach() {
    let mut p = quests_at_start(1);
    p.profile.to_level = 10;
    p.world.terrain = crate::terrain::Terrain::default();
    let mut camp = p.model.start.clone();
    camp.pos.x += 100.0;
    p.model.quests[0].xp = 2000;
    p.model.quests[0].objectives = vec![Objective {
        loc: camp,
        text: "Kill the chief".into(),
        kills: 1.0,
        mob_level: 7,
        uses: 0.0,
        extra: 0.0,
        dungeon: None,
        elite: false,
        count: 1.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 1,
        pack: 1.0,
        spots: None,
        guard: Guard::default(),
        beside_giver: false,
        item: false,
        alts: vec![],
        after: 0,
        handed_by: vec![],
        child: None,
        child_name: None,
        vendors: vec![],
        auction: vec![],
        slot: None,
        stock: None,
    }];
    let route = planner(&p).greedy();
    assert!(route.contains(&Stop::quest(0, Kind::TurnIn)), "{route:?}");
}

/// A trip flies to a port before its boat, and flies on from where a boat lands, when that is
/// faster: Ironforge to Theramore by the flight to Menethil Harbor and the boat, and back.
#[test]
fn trips_fly_to_a_boat_and_from_it() {
    use super::state::Leg;
    let mut p = quests_at_start(0);
    p.params.gold_weight = 0.0;
    let planner = planner(&p);
    let trips = planner.trips();
    let node = |name: &str| {
        (p.world.taxi_nodes.iter())
            .position(|n| n.name.starts_with(name))
            .unwrap()
    };
    let (ironforge, menethil, theramore) = (node("Ironforge"), node("Menethil"), node("Theramore"));
    let boat = (p.world.links.iter())
        .position(|l| l.name == "Boat Menethil Harbor - Theramore")
        .unwrap();
    let back = (p.world.links.iter())
        .rposition(|l| l.name == "Boat Menethil Harbor - Theramore")
        .unwrap();
    let known = (1u128 << ironforge) | (1u128 << menethil);
    let (a, b) = (p.world.taxi_nodes[ironforge].pos, p.world.taxi_nodes[theramore].pos);
    let (with_flights, leg) = trips.leg(&a, &b, 30, known, 0);
    assert!(
        matches!(leg, Leg::FlyLink(f, t, k, l) if (f, t, k, l) == (ironforge, menethil, boat, boat)),
        "{leg:?}"
    );
    let (by_boat, _) = trips.leg(&a, &b, 30, 1u128 << ironforge, 0);
    assert!(with_flights < by_boat, "{with_flights} vs {by_boat}");
    let (_, leg) = trips.leg(&b, &a, 30, known, 0);
    assert!(
        matches!(leg, Leg::LinkFly(k, l, f, t) if (k, l, f, t) == (back, back, menethil, ironforge)),
        "{leg:?}"
    );
}

/// A kill's loot fills the bags (`Params::loot_per_kill`) up to the room a level 1 character
/// has; what no longer fits costs `bag_weight` a slot, and quest items take room too.
#[test]
fn loot_fills_the_bags_and_what_no_longer_fits_costs() {
    let p = quests_at_start(1);
    let planner = planner(&p);
    let mut s = planner.initial_state();
    let (room, each) = (planner.growth().bag_room(1), p.params.loot_per_kill);
    planner.growth().fill_bags(&mut s, room / each / 2.0);
    assert!((f64::from(s.bags) - room / 2.0).abs() < 1e-3, "half full: {}", s.bags);
    assert!(s.spent.bag_overflow.abs() < 1e-9, "all of it fits");
    s.held = 2;
    planner.growth().fill_bags(&mut s, room / each);
    assert!(
        (f64::from(s.bags) - (room - 2.0)).abs() < 1e-3,
        "full but the quest items: {}",
        s.bags
    );
    let lost = room / 2.0 + 2.0;
    assert!(
        (s.spent.bag_overflow - lost * p.params.bag_weight).abs() < 1e-3,
        "{}",
        s.spent.bag_overflow
    );
}

/// An item given on accepting takes a slot until the turn-in.
#[test]
fn quest_items_take_a_slot_until_the_turn_in() {
    let mut p = quests_at_start(1);
    p.model.quests[0].carried = true;
    let planner = planner(&p);
    let mut s = planner.initial_state();
    planner.accept(&mut s, 0);
    assert_eq!(s.held, 1);
    planner.turn_in(&mut s, 0);
    assert_eq!(s.held, 0);
}

/// Bags half full by a vendor: the loot is sold there, a step of the guide; nearly empty, no.
#[test]
fn a_vendor_on_the_way_empties_half_full_bags() {
    let mut p = quests_at_start(1);
    let mut vendor = p.model.start.clone();
    vendor.pos.x += 30.0;
    p.model.vendors = vec![vendor];
    let planner = planner(&p);
    let mut s = planner.initial_state();
    let mut events = Vec::new();
    s.bags = 2.0;
    planner.sell(&mut s, &mut |_, e| events.push(e));
    assert!(events.is_empty() && s.bags > 0.0, "not worth a stop yet");
    s.bags = (planner.growth().bag_room(1) * 0.6) as f32;
    let before = s.time;
    planner.sell(&mut s, &mut |_, e| events.push(e));
    assert!(
        matches!(events[..], [super::state::Event::Sell { vendor: 0 }]),
        "{events:?}"
    );
    assert!(s.bags == 0.0 && s.time >= before + p.params.sell_time);
}

/// What the bags keep for the class and the gathering professions: a hunter's quiver takes a bag,
/// a herbalist keeps a share for the herbs from the level it is taken.
#[test]
fn the_class_and_gathering_keep_bag_room() {
    let mut p = quests_at_start(1);
    let warrior = planner(&p).growth().bag_room(30);
    assert!(
        (warrior - (crate::params::bag_slots(30) - 4.0)).abs() < 1e-9,
        "{warrior}"
    );
    p.profile.class_id = 3;
    let hunter = planner(&p).growth().bag_room(30);
    assert!(
        (warrior - hunter - (crate::params::bag_slots(30) - 16.0) / 4.0).abs() < 1e-9,
        "{hunter}"
    );
    p.profile.class_id = 1;
    p.profile.professions = vec![crate::profession::ProfessionGoal {
        key: "herbalism".into(),
        target: 300,
        start_level: Some(10),
        milestones: vec![],
    }];
    assert!(
        (planner(&p).growth().bag_room(5) - (crate::params::bag_slots(5) - 4.0)).abs() < 1e-9,
        "not taken yet"
    );
    let herbalist = planner(&p).growth().bag_room(30);
    assert!(
        (warrior - herbalist - p.params.gather_share * crate::params::bag_slots(30)).abs() < 1e-9,
        "{herbalist}"
    );
}

/// A timed quest whose mob is too strong for the character is taken once it is strong enough
/// for it (the grinding comes before the clock starts), and turned in in time; untimed, the same
/// quest is taken at once and its mob waited for.
#[test]
fn timed_quest_taken_once_strong_enough_for_its_objectives() {
    let mut p = quests_at_start(1);
    p.profile.to_level = 20;
    let mut loc = p.model.start.clone();
    loc.pos.x += 50.0;
    p.model.quests[0].objectives = vec![Objective {
        loc,
        text: "Kill a strong mob".into(),
        kills: 1.0,
        mob_level: 12,
        uses: 0.0,
        extra: 0.0,
        dungeon: None,
        elite: false,
        count: 1.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 1,
        pack: 1.0,
        spots: None,
        guard: Guard::default(),
        beside_giver: false,
        item: false,
        alts: vec![],
        after: 0,
        handed_by: vec![],
        child: None,
        child_name: None,
        vendors: vec![],
        auction: vec![],
        slot: Some(1),
        stock: None,
    }];
    // Of the mob's level (not gray once there), offered from level 1.
    (p.model.quests[0].xp, p.model.quests[0].level) = (100, 12);
    let route = [
        Stop::quest(0, Kind::Accept),
        Stop::quest(0, Kind::Objective(0)),
        Stop::quest(0, Kind::TurnIn),
    ];
    let gates = |p: &Prepared| {
        let planner = planner(p);
        let s = planner.start_state(planner.planned(&route));
        (planner.gate_level(&s, route[0]), planner.gate_level(&s, route[1]))
    };
    let (accept, objective) = gates(&p);
    assert!(
        objective > 1 && accept == 1,
        "untimed: taken at once ({accept}, {objective})"
    );
    p.model.quests[0].time_limit = Some(600.0);
    let (accept, objective) = gates(&p);
    assert_eq!(accept, objective, "timed: taken at the mob's level");
    let mut trace = Vec::new();
    planner(&p).simulate(&route, Some(&mut trace)).expect("a valid route");
    let at = |kind: Kind| {
        trace.iter().find_map(|t| match t.event {
            super::state::Event::Stop { stop, .. } if stop.kind == kind => Some(t.level),
            _ => None,
        })
    };
    assert!(
        at(Kind::Accept).is_some_and(|level| level >= objective),
        "{:?}",
        at(Kind::Accept)
    );
    assert!(at(Kind::TurnIn).is_some(), "turned in in time");
    assert!(
        !trace
            .iter()
            .any(|t| matches!(t.event, super::state::Event::Abandon { .. })),
        "not given up"
    );
}

/// Two players: quest 0 is still for both, quest 1 only for the character (the other did it).
fn duo(xp: i64) -> Prepared {
    let mut p = quests_at_start(2);
    for q in &mut p.model.quests {
        q.xp = xp;
    }
    p.model.initial.mates = vec![crate::model::MateStart {
        level: 1,
        ..crate::model::MateStart::default()
    }];
    p.model.initial.need = vec![0b11, 0b01];
    p
}

const BOTH: [Stop; 4] = [
    Stop {
        index: 0,
        kind: Kind::Accept,
    },
    Stop {
        index: 0,
        kind: Kind::TurnIn,
    },
    Stop {
        index: 1,
        kind: Kind::Accept,
    },
    Stop {
        index: 1,
        kind: Kind::TurnIn,
    },
];

/// Each player gets the XP of the quests still for it, the kills' XP at its own level. The
/// lowest one is the character planned for (`State::order`).
#[test]
fn each_player_gets_the_xp_of_its_quests() {
    let mut p = duo(100);
    p.params.group_sync_max = 10.0;
    let planner = planner(&p);
    let mut s = planner.start_state(planner.planned(&BOTH));
    for stop in BOTH {
        planner.apply(&mut s, stop, None);
    }
    assert_eq!(s.party(), [(1, 200), (1, 100)]);
    assert_eq!((s.level, s.xp), (1, 100), "the one behind is planned for");
    planner.growth().gain_kills(&mut s, 50);
    assert_eq!(s.party(), [(1, 250), (1, 150)], "same level: the same kill XP");
}

/// The guide is over once everyone reached the target level, not the character alone.
#[test]
fn the_guide_waits_for_everyone() {
    let p = duo(0);
    let planner = planner(&p);
    let mut s = planner.initial_state();
    s.level = 2;
    assert!(!planner.ended(&s), "the other player is still level 1");
    s.mates[0].level = 2;
    assert!(planner.ended(&s));
}

/// The time the players are levels apart beyond the window costs the score; within it nothing.
#[test]
fn levels_apart_cost_the_score() {
    let mut p = duo(200);
    (p.params.group_sync_window, p.params.group_sync_max) = (0.05, 10.0);
    p.model.quests[0].start_uses = 60.0;
    let route = [BOTH[2], BOTH[3], BOTH[0], BOTH[1]];
    let planner = planner(&p);
    let (_, apart) = planner.simulate_full(&route, None).unwrap();
    // Quest 1 puts the character 200 XP ahead (a level is 400 at level 1): half a level apart
    // while taking quest 0, beyond the window.
    let bar = planner.rules().to_next_level(1) as f64;
    let over = 200.0 / bar - p.params.group_sync_window;
    assert!(apart.desync > 0.0, "{apart:?}");
    assert!(
        apart.desync <= over * (apart.total + 1.0),
        "{} for {over} levels",
        apart.desync
    );
    p.model.initial.need = vec![0b11, 0b11];
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let (_, together) = planner.simulate_full(&route, None).unwrap();
    assert_eq!(together.desync, 0.0);
}

/// Together once, then further apart than `group_sync_max`: the player behind kills mobs on
/// its own until it is within half the window of the one ahead. Below the most, nothing.
#[test]
fn the_player_behind_catches_up() {
    let mut p = duo(100);
    (p.params.group_sync_window, p.params.group_sync_max) = (0.05, 0.1);
    let planner = planner(&p);
    let mut s = planner.start_state(planner.planned(&BOTH));
    let mut events = Vec::new();
    for stop in BOTH {
        planner.apply(&mut s, stop, Some(&mut events));
    }
    let caught = events.iter().find_map(|t| match t.event {
        super::Event::CatchUp { members, seconds, .. } => Some((members, seconds)),
        _ => None,
    });
    let (members, seconds) = caught.expect("a catch-up");
    assert_eq!(members, 0b10, "the other player, behind");
    assert!(seconds > 0.0);
    assert!(planner.growth().spread(&s) <= 0.05, "{}", planner.growth().spread(&s));
    assert_eq!(s.party()[0], (1, 200), "the character waits");
    p.params.group_sync_max = 0.3;
    let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
    let mut s = planner.start_state(planner.planned(&BOTH));
    for stop in BOTH {
        planner.apply(&mut s, stop, None);
    }
    assert_eq!(
        s.party(),
        [(1, 200), (1, 100)],
        "a quarter of a level apart: within the most"
    );
}

#[test]
fn cat_form_is_a_quest_reward_before_it_can_change_travel_or_combat() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let req:PlanRequest=serde_json::from_value(serde_json::json!({"race":"skyborne_horde","class":"druid","from_level":20,"to_level":21,"required_class_quests":[],"start":{"level":20}})).unwrap();
    let mut original = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
    let power = original.model.power.take();
    let mut p = at_start(original, 2);
    p.model.power = power;
    let a = crate::world::Pos {
        continent: 1,
        x: -100.0,
        y: -100.0,
        zone: 0,
        z: None,
    };
    p.model.initial.pos = Some((a, 0));
    p.world.terrain = crate::terrain::movement_test_ground(false);
    (p.params.run_speed, p.params.detour) = (7.0, 1.0);
    p.model.quests[0].id = 98362; // The final Horde Cat Form hand-in, after its trial.
    p.model.index = p.model.quests.iter().enumerate().map(|(i, q)| (q.id, i)).collect();
    for q in &mut p.model.quests {
        q.level = 20;
        q.starts[0].pos = a;
        q.ends[0].pos = a;
    }
    p.model.quests[1].xp = 100_000;
    p.model.quests[1].starts[0].pos = crate::world::Pos { x: -400.0, ..a };
    p.model.quests[1].ends[0].pos = crate::world::Pos { x: -400.0, ..a };
    let mut trainer = p.model.start.clone();
    trainer.pos = a;
    p.model.trainers = vec![trainer];
    let edits = |i| {
        [
            super::Edit::Stop {
                stop: Stop::quest(i, Kind::Accept),
                farm: false,
                spot: None,
            },
            super::Edit::Stop {
                stop: Stop::quest(i, Kind::TurnIn),
                farm: false,
                spot: None,
            },
        ]
    };
    let far = edits(1);
    let mut trained = vec![super::Edit::Train { trainer: 0 }];
    trained.extend(far);
    let plan = planner(&p).explicit();
    let locked = plan.edit(&trained);
    let mut unlock = edits(0).to_vec();
    unlock.extend(far);
    let learned = plan.edit(&unlock);
    assert_eq!(learned.errors(), 0);
    assert!(
        (locked.breakdown.travel - 42.85714).abs() < 0.01,
        "training cannot buy Cat Form: {}",
        locked.breakdown.travel
    );
    assert!(
        (learned.breakdown.travel - 32.96703).abs() < 0.01,
        "the completed quest enables Feral Swiftness"
    );
    let guide = crate::export::build_traced(
        &plan,
        &p.profile,
        &learned.route,
        &learned.trace,
        learned.breakdown.total,
        learned.breakdown,
    );
    assert!(
        guide.steps[2].power > guide.steps[0].power,
        "combat power waits for the quest too"
    );
    assert!(
        !guide.spells.contains(&768),
        "quest-only spells are not trainer purchases"
    );
    let end = plan.end_of(&learned.route).unwrap();
    let after = job::StartState::default().after(&p.model, &p.world, Edition::Forever.rules(), &end);
    assert!(
        after.spells.contains(&768),
        "the continuing character knows the quest reward"
    );
    // Starting at level 20 cannot stand in for the quest. Recorded spells and confirmed
    // completions can, even when the player starts a new guide after finishing the chain.
    for (start, expected) in [
        (serde_json::Value::Null, locked.breakdown.travel),
        (serde_json::json!({"level":20,"spells":[768]}), learned.breakdown.travel),
        (
            serde_json::json!({"level":20,"completed":[98362]}),
            learned.breakdown.travel,
        ),
    ] {
        let mut request = serde_json::to_value(&req).unwrap();
        request["start"] = start;
        let request = serde_json::from_value(request).unwrap();
        let imported = job::prepare(&conn, &overrides, &request, &|_| {}).unwrap();
        p.model.initial.abilities = imported.model.initial.abilities;
        let resumed = planner(&p).explicit().edit(&far);
        assert!((resumed.breakdown.travel - expected).abs() < 0.01);
    }
    // A synthetic leveled preset assumes earlier unlocks, except for a quest explicitly
    // requested as still to do. Recorded characters never infer missing spells this way.
    let mut synthetic = req.clone();
    synthetic.start = None;
    synthetic.from_level = 21;
    synthetic.to_level = 22;
    p.profile.from_level = 21;
    p.profile.to_level = 22;
    for required in [false, true] {
        if required {
            synthetic.required_quests.push(job::RequiredQuest::Id(98362));
        }
        let preset = job::prepare(&conn, &overrides, &synthetic, &|_| {}).unwrap();
        p.model.initial.abilities = preset.model.initial.abilities;
        let resumed = planner(&p).explicit().edit(&far);
        let expected = if required { 42.85714 } else { 32.96703 };
        assert!(
            (resumed.breakdown.travel - expected).abs() < 0.01,
            "an explicit quest requirement cannot inherit its own reward from the preset"
        );
    }
    p.profile.from_level = 20;
    p.profile.to_level = 21;
    p.model.initial.abilities = 0;
    p.params.assume_movement_talents = false;
    let without = planner(&p).explicit().edit(&unlock);
    assert!(
        (without.breakdown.travel - locked.breakdown.travel).abs() < 0.01,
        "without the talent Cat Form has no speed bonus"
    );
}

#[test]
fn late_form_unlock_does_not_credit_unlearned_attack_ranks() {
    let conn = database_of("forever");
    // Isolate each real form-dependent attack so another family's gain cannot hide premature credit
    // or a later rank being treated as the quest's free starter rank.
    for (family, rank, quest_id) in [
        ("Claw", 3029, 98362),
        ("Rip", 9492, 98362),
        ("Rake", 1822, 98362),
        ("Shred", 5221, 98362),
        ("Tiger's Fury", 5217, 98362),
        ("Ferocious Bite", 22568, 98362),
        ("Maul", 6808, 94638),
        ("Swipe", 779, 94638),
    ] {
        let mut overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
        overrides
            .spells
            .get_mut("druid")
            .unwrap()
            .retain(|name, _| name == family);
        let req: PlanRequest = serde_json::from_value(serde_json::json!({
            "race":"skyborne_horde", "class":"druid", "from_level":40, "to_level":41,
            "required_class_quests":[], "start":{"level":40}
        }))
        .unwrap();
        let mut original = job::prepare(&conn, &overrides, &req, &|_| {}).unwrap();
        let power = original.model.power.take();
        let mut p = at_start(original, 1);
        p.model.power = power;
        p.model.trainers.push(p.model.start.clone());
        p.model.quests[0].id = quest_id;
        p.model.quests[0].level = if quest_id == 98362 { 20 } else { 10 };
        p.model.index = [(quest_id, 0)].into_iter().collect();
        let quest = [
            super::Edit::Stop {
                stop: Stop::quest(0, Kind::Accept),
                farm: false,
                spot: None,
            },
            super::Edit::Stop {
                stop: Stop::quest(0, Kind::TurnIn),
                farm: false,
                spot: None,
            },
        ];
        let train = super::Edit::Train { trainer: 0 };
        let plan = planner(&p).replaying().explicit();
        let before = plan.edit(&[train, quest[0], quest[1]]);
        let after = plan.edit(&[quest[0], quest[1], train]);
        let export = |edited: &super::Edited| {
            crate::export::build_traced(
                &plan,
                &p.profile,
                &edited.route,
                &edited.trace,
                edited.breakdown.total,
                edited.breakdown,
            )
        };
        let before_guide = export(&before);
        let after_guide = export(&after);
        let power =
            |guide: &crate::export::Route, kind| guide.steps.iter().find(|s| s.kind == kind).unwrap().power.unwrap();
        let trained_spells = |guide: &crate::export::Route| {
            guide
                .steps
                .iter()
                .filter(|s| s.kind == crate::export::StepKind::Train)
                .flat_map(|s| s.spells.iter().copied())
                .collect::<Vec<_>>()
        };
        let fully_learned = power(&after_guide, crate::export::StepKind::Train);
        assert!(
            power(&before_guide, crate::export::StepKind::Train) < fully_learned,
            "{family}: no combat share before the form unlock"
        );
        assert!(
            power(&before_guide, crate::export::StepKind::TurnIn) < fully_learned,
            "{family}: the earlier trainer visit cannot supply later combat ranks after the unlock"
        );
        assert!(
            power(&after_guide, crate::export::StepKind::TurnIn) < fully_learned,
            "{family}: the quest does not train every rank available at level 40"
        );
        assert!(
            !trained_spells(&before_guide).contains(&rank),
            "{family}: cannot train before the form"
        );
        assert!(
            trained_spells(&after_guide).contains(&rank),
            "{family}: can train after the quest at the same level"
        );
        assert!(
            !trained_spells(&after_guide).contains(&1082),
            "Claw 1 came free from the quest"
        );
        let reward = plan.end_of(&before.route).unwrap();
        let reward = job::StartState::default().after(&p.model, &p.world, Edition::Forever.rules(), &reward);
        if quest_id == 98362 {
            assert!(reward.spells.contains(&1082));
        }
        assert!(
            !reward.spells.contains(&rank),
            "{family}: the earlier trainer visit did not learn a locked rank"
        );
        if family == "Claw" {
            // The spellbook records only the highest rank; continuing guides must not rebuy it.
            let mut imported = req;
            imported.start.as_mut().unwrap().spells = vec![768, 3029, 1079];
            let imported = job::prepare(&conn, &overrides, &imported, &|_| {}).unwrap();
            p.model.initial.abilities = imported.model.initial.abilities;
            let plan = planner(&p).replaying().explicit();
            let replay = plan.edit(&[train]);
            let guide = crate::export::build_traced(
                &plan,
                &p.profile,
                &replay.route,
                &replay.trace,
                replay.breakdown.total,
                replay.breakdown,
            );
            assert!(!trained_spells(&guide).contains(&3029));
            assert!(!trained_spells(&guide).contains(&1082));
        }
    }
}

#[test]
fn undead_paladin_learns_holy_light_from_its_quest() {
    let conn = database_of("forever");
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let req: PlanRequest = serde_json::from_value(serde_json::json!({
        "race":"undead","class":"paladin","from_level":6,"to_level":7,
        "required_class_quests":[],"start":{"level":6}
    }))
    .unwrap();
    let mut p = at_start(job::prepare(&conn, &overrides, &req, &|_| {}).unwrap(), 1);
    p.model.quests[0].id = 90902; // Rediscovering the Light rewards Holy Light 1.
    p.model.index = [(90902, 0)].into_iter().collect();
    let trainer = p.model.start.clone();
    p.model.trainers.push(trainer);
    let plan = planner(&p).replaying().explicit();
    let before = plan.edit(&[super::Edit::Train { trainer: 0 }]);
    let after = plan.edit(&[
        super::Edit::Stop {
            stop: Stop::quest(0, Kind::Accept),
            farm: false,
            spot: None,
        },
        super::Edit::Stop {
            stop: Stop::quest(0, Kind::TurnIn),
            farm: false,
            spot: None,
        },
    ]);
    let spells = |route: &[Stop]| {
        let end = plan.end_of(route).unwrap();
        job::StartState::default()
            .after(&p.model, &p.world, Edition::Forever.rules(), &end)
            .spells
    };
    assert!(
        !spells(&before.route).contains(&635),
        "the trainer cannot bypass the class quest"
    );
    let before_guide = crate::export::build_traced(
        &plan,
        &p.profile,
        &before.route,
        &before.trace,
        before.breakdown.total,
        before.breakdown,
    );
    assert!(
        !before_guide
            .steps
            .iter()
            .any(|s| s.kind == crate::export::StepKind::Train && s.spells.contains(&639)),
        "a later rank cannot bypass the starter quest"
    );
    assert!(
        spells(&after.route).contains(&635),
        "another class uses the same quest reward path"
    );
    let learned = plan.edit(&[
        super::Edit::Stop {
            stop: Stop::quest(0, Kind::Accept),
            farm: false,
            spot: None,
        },
        super::Edit::Stop {
            stop: Stop::quest(0, Kind::TurnIn),
            farm: false,
            spot: None,
        },
        super::Edit::Train { trainer: 0 },
    ]);
    let guide = crate::export::build_traced(
        &plan,
        &p.profile,
        &learned.route,
        &learned.trace,
        learned.breakdown.total,
        learned.breakdown,
    );
    assert!(
        guide
            .steps
            .iter()
            .any(|s| s.kind == crate::export::StepKind::Train && s.spells.contains(&639)),
        "training after the quest teaches Holy Light 2"
    );
}

#[test]
fn classic_cat_speed_follows_the_available_feral_swiftness_rank() {
    let conn = database();
    let mut overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    overrides.params.zone_gap = 0.0; // A supported setting: zone closures do not separate cached legs.
    let req: PlanRequest = serde_json::from_value(serde_json::json!({
        "race":"nightelf","class":"druid","from_level":20,"to_level":22,
        "required_class_quests":[],"start":{"level":20,"spells":[768]}
    }))
    .unwrap();
    let mut p = at_start(job::prepare(&conn, &overrides, &req, &|_| {}).unwrap(), 2);
    let a = crate::world::Pos {
        continent: 1,
        x: -100.0,
        y: -100.0,
        zone: 0,
        z: None,
    };
    p.model.initial.pos = Some((a, 0));
    p.world.terrain = crate::terrain::movement_test_ground(false);
    (p.params.run_speed, p.params.detour) = (7.0, 1.0);
    for q in &mut p.model.quests {
        q.starts[0].pos = a;
        q.ends[0].pos = a;
        q.level = 20;
    }
    p.model.quests[0].xp = Edition::Classic.rules().to_next_level(20);
    p.model.quests[1].xp = 100_000;
    let b = crate::world::Pos { x: -400.0, ..a };
    p.model.quests[1].starts[0].pos = b;
    p.model.quests[1].ends[0].pos = b;
    let edits = |i| {
        [
            super::Edit::Stop {
                stop: Stop::quest(i, Kind::Accept),
                farm: false,
                spot: None,
            },
            super::Edit::Stop {
                stop: Stop::quest(i, Kind::TurnIn),
                farm: false,
                spot: None,
            },
        ]
    };
    let far = edits(1);
    let plan = planner(&p).explicit();
    let before = plan.edit(&far);
    let after = plan.edit(&[edits(0)[0], edits(0)[1], far[0], far[1]]);
    assert_eq!(after.errors(), 0);
    assert!(
        (before.breakdown.travel - 37.26708).abs() < 0.01,
        "rank one at level 20"
    );
    assert!(
        (after.breakdown.travel - 32.96703).abs() < 0.01,
        "the same leg at level 21 uses rank two"
    );
    p.params.assume_movement_talents = false;
    let without = planner(&p).explicit().edit(&far);
    assert!(
        (without.breakdown.travel - 42.85714).abs() < 0.01,
        "without the talent Cat Form keeps running speed"
    );
}
