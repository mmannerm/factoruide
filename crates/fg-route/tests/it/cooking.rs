//! Secondary ranks use trainers through 150, then the profession's Expert book.

use crate::common::{database_of, repo};
use fg_route::export::{self, StepKind};
use fg_route::job::{self, Overrides, PlanRequest};
use fg_route::plan::{Breakdown, Event, Planner, Timed};
use fg_route::xp::Edition;
use serde_json::json;

#[test]
fn secondary_expert_book_sources_and_exports_match_each_faction() {
    for edition in Edition::ALL {
        let conn = database_of(edition.key());
        let overrides = Overrides::load_edition(&repo().join("overrides"), edition).unwrap();
        for (key, line, item, book_name, profession_name, sellers) in [
            (
                "cooking",
                185,
                16072,
                "Expert Cookbook",
                "Cooking",
                [("orc", 12033, 405, "Wulan"), ("human", 3955, 331, "Shandrina")],
            ),
            (
                "fishing",
                356,
                16083,
                "Expert Fishing - The Bass and You",
                "Fishing",
                [
                    ("orc", 2626, 33, "Old Man Heming"),
                    ("human", 2626, 33, "Old Man Heming"),
                ],
            ),
            (
                "first_aid",
                129,
                16084,
                "Expert First Aid - Under Wraps",
                "First Aid",
                [
                    ("orc", 13476, 15, "Balai Lok'Wein"),
                    ("human", 2805, 45, "Deneb Walker"),
                ],
            ),
        ] {
            for (race, vendor, zone, name) in sellers {
                let request: PlanRequest = serde_json::from_value(json!({
                    "race":race, "class":"warrior", "from_level":20, "to_level":30,
                    "required_class_quests":[], "params":{"edition":edition,"profession_trainers":true,"effort":0},
                    "professions":[{"key":key,"target":225}]
                }))
                .unwrap();
                let prepared = job::prepare(&conn, &overrides, &request, &|_| {}).unwrap();
                assert_eq!(prepared.params.edition, edition);
                let sources: Vec<_> = prepared
                    .model
                    .profession_trainers
                    .iter()
                    .enumerate()
                    .filter(|(_, source)| source.line == line)
                    .collect();
                assert!(
                    sources.iter().any(|(_, s)| s.book.is_none()),
                    "ordinary {profession_name} trainers remain"
                );
                assert!(
                    sources
                        .iter()
                        .filter(|(_, s)| s.book.is_none())
                        .all(|(_, s)| s.cap <= 150),
                    "a trainer cannot grant Expert or Artisan {profession_name}"
                );
                let books: Vec<_> = sources.iter().filter(|(_, s)| s.book.is_some()).collect();
                assert_eq!(books.len(), 1, "one vendor of the character's faction");
                let (trainer, source) = *books[0];
                assert_eq!((source.loc.id, source.loc.zone, source.cap), (vendor, zone, 225));
                assert_eq!(source.book.unwrap().item, item);

                let planner = Planner::new(&prepared.model, &prepared.world, &prepared.params, &prepared.profile);
                let trace = [Timed {
                    event: Event::ProfTrain {
                        profession: 0,
                        trainer,
                        ranks: 0b100,
                        outbound: 0.0,
                    },
                    time: 30.0,
                    level: 20,
                    xp: 0,
                    power: 20.0,
                    gear: 0.0,
                    party: vec![],
                }];
                let route = export::build_traced(&planner, &prepared.profile, &[], &trace, 30.0, Breakdown::default());
                let step = route.steps.iter().find(|s| s.kind == StepKind::Profession).unwrap();
                assert_eq!((step.line, step.skill), (Some(line), Some(225)));
                assert_eq!(step.target.as_ref().unwrap().id, vendor);
                assert_eq!(step.zone_id, Some(zone));
                assert_eq!(
                    step.text,
                    format!("Buy {book_name} from {name} and read it to raise your {profession_name} skill cap to 225")
                );
                let lua = export::to_lua(std::slice::from_ref(&route));
                assert!(lua.contains(book_name));
                assert!(lua.contains(&format!("line = {line}")) && lua.contains("sk = 225"));
                let encoded = serde_json::to_value(&route).unwrap();
                let restored: fg_route::Route = serde_json::from_value(encoded).unwrap();
                assert_eq!(
                    restored
                        .steps
                        .iter()
                        .find(|s| s.kind == StepKind::Profession)
                        .unwrap()
                        .say[0]
                        .args["item"],
                    export::Arg::Item(item, book_name.to_owned())
                );
                let french = export::render(&route, &fg_route::model::Names::new("frFR"));
                assert!(
                    french
                        .steps
                        .iter()
                        .find(|s| s.kind == StepKind::Profession)
                        .unwrap()
                        .text
                        .starts_with("Acheter ")
                );
            }
        }
    }
}

#[test]
fn actual_trainer_ranks_match_the_supported_editions() {
    for edition in Edition::ALL {
        let conn = database_of(edition.key());
        let overrides = Overrides::load_edition(&repo().join("overrides"), edition).unwrap();
        let request: PlanRequest = serde_json::from_value(json!({
            "race":"orc", "class":"warrior", "from_level":20, "to_level":30,
            "required_class_quests":[], "params":{"edition":edition,"profession_trainers":true,"effort":0},
            "professions":[{"key":"fishing","target":225}]
        }))
        .unwrap();
        let prepared = job::prepare(&conn, &overrides, &request, &|_| {}).unwrap();
        assert_eq!(prepared.params.edition, edition);
        let cap = |id| {
            prepared
                .model
                .profession_trainers
                .iter()
                .find(|source| source.loc.id == id && source.book.is_none())
                .map(|source| source.cap)
        };
        assert_eq!(cap(3028), Some(150), "Kah Mistrunner teaches only through Journeyman");
        if edition == Edition::Tbc {
            assert_eq!(cap(3355), Some(300), "TBC Saru teaches all ranks through Artisan");
            assert_eq!(cap(1383), None, "Snarl is no longer a rank trainer in TBC");
        } else {
            for (id, rank_cap) in [
                (10266, 75),
                (1383, 150),
                (3355, 225),
                (2836, 300),
                (11046, 75),
                (3347, 150),
                (4611, 225),
                (1386, 300),
            ] {
                assert_eq!(cap(id), Some(rank_cap), "NPC {id} in {edition:?}");
            }
        }
        assert_eq!(
            cap(2114),
            Some(300),
            "Faruza teaches all Herbalism ranks despite the Apprentice title"
        );
        for id in [8126, 7868, 7871, 4578, 11177] {
            assert_eq!(
                cap(id),
                None,
                "specialty NPC {id} must not be used to learn a profession rank"
            );
        }
        let mut alliance_request = request.clone();
        alliance_request.race = "human".into();
        let alliance = job::prepare(&conn, &overrides, &alliance_request, &|_| {}).unwrap();
        for (id, line) in [(28694, 164), (246795, 171)] {
            let trainer = alliance
                .model
                .profession_trainers
                .iter()
                .find(|trainer| trainer.loc.id == id && trainer.book.is_none());
            assert_eq!(
                trainer.map(|trainer| (trainer.line, trainer.cap)),
                (edition == Edition::Forever).then_some((line, 225)),
                "Alard and Linzy teach through Expert in Forever"
            );
            assert_eq!(cap(id), None, "these Alliance trainers are not offered to Horde");
        }
    }
}
