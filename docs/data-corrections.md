# Data corrections

The fork uses upstream's bundled databases without editing their archives. The current Forever
import records identify QuestieDB `aa1d7e7294`, Questie `90417a4c7d`, AllTheThings `aa43dec3b6`,
EG Link `45f58b070b`, and client cache builds `70790` and `70862` in the [upstream snapshot](https://github.com/brouznouf/factoruide/blob/c27f59d1804367fe367407b74483ac89a5d4f64b/data/forever.sqlite.zst).
The archive's `import_run` table and [NOTICE](../NOTICE.md) record the complete source provenance.

Source-data corrections live in `overrides/database/`; upstream's database builder reapplies them
at each build. The builder is not included in this checkout. The fork's remaining runtime
corrections add behavior or facts the bundled data does not yet supply:

| Correction | Source and behavior |
| --- | --- |
| Call of Fire | The Skyborne Horde [ritual and torch delivery](https://www.wowhead.com/forever/quest=97257/call-of-fire) uses two neutral NPC credits as interactions. The guide follows the quest's instructions and retains its six-minute limit. |
| Cat relic trials | The [Skyborne](https://www.wowhead.com/forever/quest=98404/the-great-windborne-cat-spirit), [Tauren](https://www.wowhead.com/forever/quest=98342/the-great-cat-spirit) and [Night elf](https://www.wowhead.com/forever/quest=98396/the-great-cat-spirit) trials use the entrance statue's temporary stealth to collect relics. Guards keep their actual levels. The guide asks the player to renew the blessing; expiry and detection failures are not simulated. |
| Druid training | [Blizzard's October 1](https://us.forums.blizzard.com/en/wow/t/2360696/4) and [October 15](https://us.forums.blizzard.com/en/wow/t/2360696/6) updates and [client spell levels](https://wago.tools/db2/SpellLevels?build=1.60.1.70532&useHotfixes=1): Shred rank 1 at level 20, Tiger's Fury omitted from training estimates. |
| Wind bridge | [Path 12018](https://wago.tools/db2/TaxiPathNode?build=1.60.1.70532&useHotfixes=1) is one way and available while the faction's Confront Lorthuna is held. The bundled client path now contains the same geometry, but its node is not a faction flight node, so ordinary flight routing does not expose it. |
| Shipping Label | Requiring #98247 (Alliance) or #98248 (Horde) adds an Auction House stop for one Apprentice Herbs crate and 20 Peacebloom per character needing a label, then filling it before accepting the quest. Sources are linked in `overrides/forever/quests.toml`. |

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

After a database refresh, review every `import_run` record and the source credits in
[NOTICE](../NOTICE.md). Check changed facts and remove runtime corrections the baseline now
supplies. Run the affected guide tests and `fg audit`; audit acceptance files record exceptions
rather than correcting data.
Fork-specific corrections apply only to Forever. Every edition retains upstream's bundled archive.
