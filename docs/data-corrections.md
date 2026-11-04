# Data corrections

The fork uses upstream's bundled databases and applies the edition's corrections when
loading a guide. Corrections remain readable and apply after database refreshes; the database
archives are not edited.

Selecting Shipping Label (#98247 Alliance, #98248 Horde) as a must-have adds an Auction House
stop for one Apprentice Herbs crate and 20 Peacebloom per character who still needs a label,
then filling it before accepting the quest. The first filled crate supplies the label. Use
owned items instead of buying when possible; the estimate assumes purchases, auction stock
and affordability, using the usual purchase-time estimate. Random drop rates, other crate
bundles and repeat deliveries are not modeled. Sources are linked in
`overrides/forever/quests.toml`.

Forever's bundled import records identify QuestieDB `8f55e69`, Questie `854582a` and
AllTheThings `816a63e`. The baseline now includes the ingot alternatives, both Black Shield
prerequisites and the removal of Fool's Stout; their duplicate runtime overrides are removed.
The remaining model corrections and reference data use these sources:

| Source | Applied rules |
| --- | --- |
| [QuestieDB cb187df…5c094f6](https://github.com/Questie/QuestieDB/compare/cb187dffa957157d3c93a4696000b62ea4539dd9...5c094f6e9a264f0dc54e2fde9731a37a97ed197c) | Remove the stale Fool's Stout child link from Zanzil's Mixture. The other chain corrections are in the bundled baseline. |
| [Cat relic trial](https://www.wowhead.com/forever/quest=98404/the-great-windborne-cat-spirit), [Tauren](https://www.wowhead.com/forever/quest=98342/the-great-cat-spirit), [Night elf](https://www.wowhead.com/forever/quest=98396/the-great-cat-spirit) | Use the entrance statue's temporary stealth to collect the relics. NPC levels stay unchanged. |
| [Blizzard October 1](https://us.forums.blizzard.com/en/wow/t/2360696/4), [October 15](https://us.forums.blizzard.com/en/wow/t/2360696/6), [build 70532 spell levels](https://wago.tools/db2/SpellLevels?build=1.60.1.70532&useHotfixes=1) | Shred rank 1 at level 20; Tiger's Fury removed from training estimates. Berserker Rage at level 30 is recorded for reference only; the planner does not currently track this ability. |
| [Build 70532 taxi paths](https://wago.tools/db2/TaxiPathNode?build=1.60.1.70532&useHotfixes=1), [Peacekeeper](https://www.wowhead.com/forever/npc=256635/peacekeeper) | Wind bridge path 12018: one way, while the faction's Confront Lorthuna is held. Wait for the wind before crossing. |
| [AllTheThings 94cb3ab…130b0c9](https://github.com/ATTWoWAddon/AllTheThings/compare/94cb3ab30be5a7357819d6e1869f51b538d42bd6...130b0c906e64d4a67832ee024c9346b3c24d4fae) | Fifteen profession source changes in `data/forever-profession-skills.csv`, including camp-feature skill thresholds and a removed skill-line entry. These do not change profession simulation. |

Run `python3 scripts/refresh-forever-data.py` to regenerate `overrides/forever/client.toml`
offline from `data/forever-client-facts.json`. This small reviewed snapshot records the consumed
spell levels and transport geometry, with source URLs, retrieval date and original download hashes.
Run the script with `--refresh` to check current sources: unrelated rows or CSV formatting do not
invalidate the facts; a changed consumed fact fails with a diff for review. The profession CSV is
reference data only, checked against the pinned ATT revisions during a refresh. Downloads stay in
ignored `data/cache/`. `python3 scripts/test_forever_data.py` checks extraction and regeneration.
The hotfix overlay is required: the base client export lacks the wind bridge. Its ride is
estimated at the default flight speed, plus the normal transport wait; event downtime and
actual crossing time remain unverified in production.

Import corrections belong in `overrides/database/`, which upstream's database builder reapplies
at each build. Its builder is not included in this checkout. Runtime overlays here add planner
behavior the importer format does not express: granted spell lists, blessing steps, Shipping
Label acquisition and client transport rules.

For a refresh, read `import_run` in the new database and compare the source revisions with
those linked above. Review changed facts individually, update the edition's overrides and run
the affected guide tests and `fg audit`. Remove corrections when the new baseline already
supplies them. Audit acceptance files record reviewed exceptions; they do not fix data.

Trial blessing steps are appended so the original relic objective indices remain stable. Older
fork guides that already used the inserted blessing layout should be recomputed before editing.

The fork-specific corrections apply to Forever; every edition retains upstream's bundled
archive. Production gameplay has not been validated; the trial estimate assumes the player
maintains the blessing.
