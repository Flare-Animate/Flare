"""docs/surfaces.md lists, for each feature, what each of the four surfaces does, and AGENTS.md points
to it. Run with `python3 -m unittest discover -s scripts/tests -p 'test_surfaces.py'`.
"""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ALLOWED = ("yes", "no", "n/a", "partly")


def rows():
    out = []
    for line in (ROOT / "docs/surfaces.md").read_text().splitlines():
        if line.startswith("|") and not line.startswith("|---") and not line.startswith("| Feature"):
            out.append([cell.strip() for cell in line.strip().strip("|").split("|")])
    return out


class Surfaces(unittest.TestCase):
    def test_every_row_says_what_each_of_the_four_surfaces_does(self):
        table = rows()
        self.assertGreater(len(table), 8)
        for row in table:
            self.assertEqual(len(row), 6, row)
            for cell in row[1:5]:
                word = re.sub(r"[*]|\(.*\)", "", cell).strip()
                self.assertIn(word, ALLOWED, f"{row[0]}: {cell!r}")

    def test_a_gap_says_why_and_what_closes_it(self):
        for row in rows():
            if any("no" == re.sub(r"[*]", "", c).strip() for c in row[1:5]):
                self.assertGreater(len(row[5]), 15, f"{row[0]} has a gap and no reason")

    def test_the_rule_is_in_agents_md_and_points_here(self):
        agents = (ROOT / "AGENTS.md").read_text()
        self.assertIn("## Every surface", agents)
        self.assertIn("docs/surfaces.md", agents)


if __name__ == "__main__":
    unittest.main()
