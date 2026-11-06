#!/usr/bin/env python3
"""Checks for the reviewed Forever source extraction and offline generator."""

import copy
import json
import runpy
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = runpy.run_path(str(ROOT / "scripts/refresh-forever-data.py"))


def source_rows():
    return {
        "SpellLevels": [
            {"SpellID": "5221", "DifficultyID": "0", "SpellLevel": "20"},
            {"SpellID": "18499", "DifficultyID": "0", "SpellLevel": "30"},
        ],
        "TaxiPath": [{"ID": "12018", "FromTaxiNode": "3361", "ToTaxiNode": "3362"}],
        "TaxiNodes": [{"ID": "3361", "ContinentID": "2991"}],
        "TaxiPathNode": [
            {"PathID": "12018", "NodeIndex": "1", "Loc_0": "20", "Loc_1": "30", "Loc_2": "0"},
            {"PathID": "12018", "NodeIndex": "0", "Loc_0": "10", "Loc_1": "15", "Loc_2": "0"},
        ],
        "UiMapAssignment": [{"UiMapID": "2665", "MapID": "2991", "Region_0": "0", "Region_1": "0", "Region_3": "100", "Region_4": "100"}],
    }


class ReviewedSources(unittest.TestCase):
    def test_unrelated_hotfixes_do_not_change_the_reviewed_facts(self):
        before = source_rows()
        after = copy.deepcopy(before)
        after["SpellLevels"].append({"SpellID": "1", "DifficultyID": "0", "SpellLevel": "99"})
        after["TaxiPathNode"].append({"PathID": "1", "NodeIndex": "0", "Loc_0": "99", "Loc_1": "99", "Loc_2": "99"})
        expected = SCRIPT["client_facts"](before)
        self.assertEqual(expected["spell_levels"], {"5221": 20})
        self.assertEqual(expected["bridge"]["points"], [[10.0, 15.0, 0.0], [20.0, 30.0, 0.0]])
        SCRIPT["unchanged"]("Client facts", expected, SCRIPT["client_facts"](after))

    def test_a_changed_used_row_requires_review(self):
        before = source_rows()
        after = copy.deepcopy(before)
        after["SpellLevels"][0]["SpellLevel"] = "22"
        with self.assertRaisesRegex(ValueError, "Client facts changed; review"):
            SCRIPT["unchanged"]("Client facts", SCRIPT["client_facts"](before), SCRIPT["client_facts"](after))

    def test_the_committed_snapshot_regenerates_the_client_overrides_offline(self):
        snapshot = json.loads((ROOT / "data/forever-client-facts.json").read_text())
        generated = SCRIPT["corrections"](snapshot)
        self.assertEqual(generated, (ROOT / "overrides/forever/client.toml").read_text())



if __name__ == "__main__":
    unittest.main()
