# Data corrections

The fork uses upstream's bundled databases and applies the edition's `quests.toml` when
loading a guide. Corrections remain readable and apply after database refreshes; the database
archives are not edited.

Forever's bundled import records identify QuestieDB `cb187df` and Questie `ed1f405`. The
corrections here reconcile the following newer changes:

| Source | Applied rules |
| --- | --- |
| [QuestieDB cb187df…5c094f6](https://github.com/Questie/QuestieDB/compare/cb187dffa957157d3c93a4696000b62ea4539dd9...5c094f6e9a264f0dc54e2fde9731a37a97ed197c) | The Lost Ingots and Lightforge Ingots are alternatives; Lightforge Ingots follows Lightforge Iron. The Black Shield needs both reports. Fool's Stout is not a child quest. |
| [Questie 9b46e83](https://github.com/Questie/Questie/commit/9b46e836ccbe3fb9ce394bd6415cf11db9d895fa) | Fool's Stout is unavailable. |
| [Cat relic trial](https://www.wowhead.com/forever/quest=98404/the-great-windborne-cat-spirit), [Tauren](https://www.wowhead.com/forever/quest=98342/the-great-cat-spirit), [Night elf](https://www.wowhead.com/forever/quest=98396/the-great-cat-spirit) | Use the entrance statue's temporary stealth to collect the relics. NPC levels stay unchanged. |

For a refresh, read `import_run` in the new database and compare the source revisions with
those linked above. Review changed facts individually, update the edition's overrides and run
the affected guide tests and `fg audit`. Remove corrections when the new baseline already
supplies them. Audit acceptance files record reviewed exceptions; they do not fix data.

These corrections affect Forever. Newer Classic/TBC changes and AllTheThings' level-70 druid
changes are outside this update. Production gameplay acceptance remains to be checked when
servers open; the trial estimate assumes the player maintains the blessing.
