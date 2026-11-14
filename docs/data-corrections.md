# Data corrections

The fork uses upstream's bundled databases without editing their archives. The current Forever
import records identify QuestieDB `d57637f490`, Questie `84b9da17fb`, AllTheThings `ab0d11aecd`,
EG Link `3067c69190`, and client cache builds `70790` and `70862` in the [v0.19.0 snapshot](https://github.com/brouznouf/factoruide/blob/469aa5a20e0676f3f8e01de6bbd34715e014e53a/data/forever.sqlite.zst).
The archive's `import_run` table and [NOTICE](../NOTICE.md) record the complete source provenance.

Source-data corrections live in `overrides/database/`; upstream's database builder reapplies them
at each build. The builder is not included in this checkout. The fork's remaining runtime
corrections add behavior or facts the bundled data does not yet supply:

| Correction | Source and behavior |
| --- | --- |
| Expert Cooking (all editions) | The [Expert Cookbook](https://www.wowhead.com/forever/item=16072/expert-cookbook) (#16072) requires Cooking 125 and raises the cap to 225. The bundled archives have Wulan (#12033, Horde, Desolace) and Shandrina (#3955, Alliance, Ashenvale), but omit the book and its vendor relations. `profession::EXPERT_COOKBOOK` supplies the item and faction-vendor facts; locations use the existing NPC data. Ordinary Cooking trainers teach only through cap 150. |
| The Glowing Shard (all editions) | The bundled quest text sends the player to Ratchet, but its objective row classifies Sputtervalve (#3442) as a creature credit under Wailing Caverns. The common `overrides/quests.toml` entry treats that credit as an outdoor conversation, using ordinary talk timing and translation without requiring the quest's level; nearby enemies still guard the NPC. Item #10441 still comes from Mutanus (#3654); the dungeon is needed until each eligible party member has accepted the quest. Save a fresh addon profile and regenerate existing guides. |
| Call of Fire | The Skyborne Horde [ritual and torch delivery](https://www.wowhead.com/forever/quest=97257/call-of-fire) uses two NPC credits as interactions. The guide follows the quest's instructions and retains its six-minute limit. |
| Cat relic trials | The [Skyborne](https://www.wowhead.com/forever/quest=98404/the-great-windborne-cat-spirit), [Tauren](https://www.wowhead.com/forever/quest=98342/the-great-cat-spirit) and [Night elf](https://www.wowhead.com/forever/quest=98396/the-great-cat-spirit) trials use the entrance statue's temporary stealth to collect relics. Guards keep their actual levels. The guide asks the player to renew the blessing; expiry and detection failures are not simulated. |
| Druid training | [Blizzard's October 1](https://us.forums.blizzard.com/en/wow/t/2360696/4) and [October 15](https://us.forums.blizzard.com/en/wow/t/2360696/6) updates and [client spell levels](https://wago.tools/db2/SpellLevels?build=1.60.1.70532&useHotfixes=1): Shred rank 1 at level 20, Tiger's Fury omitted from training estimates. |
| Wind bridge | [Path 12018](https://wago.tools/db2/TaxiPathNode?build=1.60.1.70532&useHotfixes=1) is one way and available while the faction's Confront Lorthuna is held. The bundled client path now contains the same geometry, but its node is not a faction flight node, so ordinary flight routing does not expose it. |
| Shipping Label | Requiring #98247 (Alliance) or #98248 (Horde), or choosing it as a quest objective, adds an Auction House stop for one Apprentice Herbs crate and 20 Peacebloom per character needing a label. Fill the crate before accepting the quest. Sources are linked in `overrides/forever/quests.toml`. |

Shipping Label estimates assume auction stock and affordability, using the ordinary purchase-time
estimate. Use owned inputs when possible. Random crate drops, other bundles and repeat deliveries
are not modeled. Already-ready imported quests do not buy their inputs again.

Run `python3 scripts/refresh-forever-data.py` to regenerate `overrides/forever/client.toml`
offline from `data/forever-client-facts.json`. Its reviewed client facts are pinned to build
`1.60.1.70532`: the consumed spell level and transport geometry, source URLs, retrieval date and
original download hashes. With
`--refresh`, the script checks current sources and rejects changed consumed facts with a diff
for review. Downloads stay in ignored `data/cache/`. Run `python3 -I scripts/test_forever_data.py`
to check extraction and regeneration.

The bridge ride is estimated at the default flight speed plus the normal transport wait; event
downtime and actual crossing time remain unverified. Trial blessing steps are appended to keep
the relic objective indices stable. Older fork guides using the inserted blessing layout should
be recomputed before editing.

For The Glowing Shard, save a fresh addon profile after accepting the quest or completing its
conversation, then regenerate the guide. Previously saved routes retain their old dungeon
steps; updating the planner does not rewrite those routes.

After a database refresh, review every `import_run` record and the source credits in
[NOTICE](../NOTICE.md). Check changed facts and remove runtime corrections the baseline now
supplies. Run the affected guide tests and `fg audit`; audit acceptance files record exceptions
rather than correcting data.
The corrections above apply to Forever, except The Glowing Shard and Expert Cooking also apply to Classic and TBC.
Every edition retains upstream's bundled archive.

Expert Cooking book visits use the existing optional profession-trainer setting and travel-time
estimate. They assume the player can buy the book; bags, auction purchases, gold affordability
and Cooking practice are not simulated. A fresh addon profile records the learned cap, avoiding
a new book trip for a character who already read it at skill 125–150. Older profiles still infer
known ranks from current skill. Regenerate saved guides to replace their old trainer directions.

The v0.19.0 Forever snapshot also incorporates the [in-game prerequisite correction](https://github.com/brouznouf/factoruide/commit/673513fa65c7641f75625eaaf8b4816c5ca5ee2d)
for Lightforge Ingots (#526): it follows The Lost Ingots (#324), rather than replacing it.
The Forever regression tests reject taking #526 before #324, allow it afterwards, and allow
requiring both quests together. Classic and TBC still reject #526 when #324 is completed.
No runtime override restores the older Forever alternative.
