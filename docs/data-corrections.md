# Data corrections

The fork uses upstream's bundled databases and applies the edition's corrections when
loading a guide. Corrections remain readable and apply after database refreshes; the database
archives are not edited.

Forever's bundled import records identify QuestieDB `cb187df` and Questie `ed1f405`. The
corrections here reconcile the following newer changes:

| Source | Applied rules |
| --- | --- |
| [QuestieDB cb187df…5c094f6](https://github.com/Questie/QuestieDB/compare/cb187dffa957157d3c93a4696000b62ea4539dd9...5c094f6e9a264f0dc54e2fde9731a37a97ed197c) | The Lost Ingots and Lightforge Ingots are alternatives; Lightforge Ingots follows Lightforge Iron. The Black Shield needs both reports. Fool's Stout is not a child quest. |
| [Questie 9b46e83](https://github.com/Questie/Questie/commit/9b46e836ccbe3fb9ce394bd6415cf11db9d895fa) | Fool's Stout is unavailable. |
| [Cat relic trial](https://www.wowhead.com/forever/quest=98404/the-great-windborne-cat-spirit), [Tauren](https://www.wowhead.com/forever/quest=98342/the-great-cat-spirit), [Night elf](https://www.wowhead.com/forever/quest=98396/the-great-cat-spirit) | Use the entrance statue's temporary stealth to collect the relics. NPC levels stay unchanged. |
| [Blizzard October 1](https://us.forums.blizzard.com/en/wow/t/2360696/4), [October 15](https://us.forums.blizzard.com/en/wow/t/2360696/6), [build 70532 spell levels](https://wago.tools/db2/SpellLevels?build=1.60.1.70532&useHotfixes=1) | Shred rank 1 at level 20, Berserker Rage at level 30; Tiger's Fury removed from training estimates. |
| [Build 70532 taxi paths](https://wago.tools/db2/TaxiPathNode?build=1.60.1.70532&useHotfixes=1), [Peacekeeper](https://www.wowhead.com/forever/npc=256635/peacekeeper) | Wind bridge path 12018: one way, while the faction's Confront Lorthuna is held. Wait for the wind before crossing. |
| [AllTheThings 94cb3ab…130b0c9](https://github.com/ATTWoWAddon/AllTheThings/compare/94cb3ab30be5a7357819d6e1869f51b538d42bd6...130b0c906e64d4a67832ee024c9346b3c24d4fae) | Fifteen profession source changes in `data/forever-profession-skills.csv`, including camp-feature skill thresholds and a removed skill-line entry. These do not change profession simulation. |

Run `python3 scripts/refresh-forever-data.py` to regenerate `overrides/forever/client.toml` and
the profession CSV. Downloads stay in ignored `data/cache/`; URLs, source revisions and SHA256
pins are in the script. A changed download fails until its changes and new pin are reviewed.
The hotfix overlay is required: the base client export lacks the wind bridge. Its ride is
estimated at the default flight speed, plus the normal transport wait; event downtime and
actual crossing time remain unverified in production.

For a refresh, read `import_run` in the new database and compare the source revisions with
those linked above. Review changed facts individually, update the edition's overrides and run
the affected guide tests and `fg audit`. Remove corrections when the new baseline already
supplies them. Audit acceptance files record reviewed exceptions; they do not fix data.

These corrections affect Forever. Classic/TBC data is unchanged. Production gameplay acceptance
remains to be checked when servers open; the trial estimate assumes the player maintains the blessing.
