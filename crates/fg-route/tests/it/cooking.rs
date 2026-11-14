//! Cooking ranks use trainers through 150, then a faction vendor's Expert Cookbook.

use crate::common::{database_of, repo};
use fg_route::export::{self, StepKind};
use fg_route::job::{self, Overrides, PlanRequest};
use fg_route::plan::{Breakdown, Event, Planner, Timed};
use fg_route::xp::Edition;
use serde_json::json;

#[test]
fn cooking_expert_book_sources_and_exports_match_each_faction() {
    for edition in Edition::ALL {
        let conn = database_of(edition.key());
        let overrides = Overrides::load_edition(&repo().join("overrides"), edition).unwrap();
        for (race, vendor, zone, name) in [("orc", 12033, 405, "Wulan"), ("human", 3955, 331, "Shandrina")] {
            let request: PlanRequest = serde_json::from_value(json!({
                "race":race, "class":"warrior", "from_level":20, "to_level":30,
                "required_class_quests":[], "params":{"profession_trainers":true,"effort":0},
                "professions":[{"key":"cooking","target":225}]
            }))
            .unwrap();
            let prepared = job::prepare(&conn, &overrides, &request, &|_| {}).unwrap();
            let sources: Vec<_> = prepared
                .model
                .profession_trainers
                .iter()
                .enumerate()
                .filter(|(_, source)| source.line == 185)
                .collect();
            assert!(
                sources.iter().any(|(_, s)| s.book.is_none()),
                "ordinary Cooking trainers remain"
            );
            assert!(
                sources
                    .iter()
                    .filter(|(_, s)| s.book.is_none())
                    .all(|(_, s)| s.cap <= 150),
                "a trainer cannot grant Expert or Artisan Cooking"
            );
            let books: Vec<_> = sources.iter().filter(|(_, s)| s.book.is_some()).collect();
            assert_eq!(books.len(), 1, "one vendor of the character's faction");
            let (trainer, source) = *books[0];
            assert_eq!((source.loc.id, source.loc.zone, source.cap), (vendor, zone, 225));
            assert_eq!(source.book.unwrap().item, 16072);

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
            assert_eq!((step.line, step.skill, step.book), (Some(185), Some(225), Some(16072)));
            assert_eq!(step.target.as_ref().unwrap().id, vendor);
            assert_eq!(step.zone_id, Some(zone));
            assert_eq!(
                step.text,
                format!("Buy Expert Cookbook from {name} and read it to raise your Cooking skill cap to 225")
            );
            let lua = export::to_lua(std::slice::from_ref(&route));
            assert!(lua.contains("book = 16072"));
            assert!(lua.contains("line = 185") && lua.contains("sk = 225"));
            let encoded = serde_json::to_value(&route).unwrap();
            let restored: fg_route::Route = serde_json::from_value(encoded).unwrap();
            assert_eq!(
                restored
                    .steps
                    .iter()
                    .find(|s| s.kind == StepKind::Profession)
                    .unwrap()
                    .book,
                Some(16072)
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
