#!/usr/bin/env python3
"""Build a hand-made (themed) course: data/maps/<id>.bin and its entry in
data/maps/maps.json, from tools/themes/<id>.py.

    python3 -I tools/make_course.py surf_100_leagues

The theme file lists its palette, its fog, its water and a build(k) that
places its pieces on a tools/coursekit.py Course and returns the course's
zones and roles. Fields of the maps.json entry that the theme file does not
make (name, author, tier, medals, things) are kept as they are, so a
re-build keeps the medal times the bot's run set.
"""
import importlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "themes"))
import coursekit  # noqa: E402

MAPS_JSON = HERE.parent / "data" / "maps" / "maps.json"


def main(map_id):
    theme = importlib.import_module(map_id)
    k = coursekit.Course([name for name, _ in theme.PALETTE])
    made = theme.build(k)
    k.write(map_id)
    maps = json.loads(MAPS_JSON.read_text())
    entry = next((m for m in maps["maps"] if m["id"] == map_id), None)
    if entry is None:
        entry = {"id": map_id, "name": map_id}
        # after the last surf course of its tier, before harder ones and bhop
        tier = getattr(theme, "TIER", 1)
        at = max((i + 1 for i, m in enumerate(maps["maps"]) if m.get("mode", "surf") == "surf" and m["tier"] <= tier),
                 default=0)
        maps["maps"].insert(at, entry)
    entry.update(made)
    entry["fall_reset"] = True
    entry["theme"] = {
        "palette": [[name, colour] for name, colour in theme.PALETTE],
        "water": theme.WATER,
        "fog": theme.FOG,
    }
    MAPS_JSON.write_text(json.dumps(maps, indent=1, ensure_ascii=False) + "\n")
    print(f"maps.json: {map_id} updated")


if __name__ == "__main__":
    main(sys.argv[1])
