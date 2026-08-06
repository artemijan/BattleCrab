#!/usr/bin/env python3
"""Cross the parser's gap map with the live-reachability index."""
import json, os
import os
from collections import defaultdict

live = json.load(open(os.environ.get("OUT", "target") + "/live.json"))
LIVE = set(int(x) for x in live["live"])
WHY = live["why"]

rows = defaultdict(lambda: defaultdict(list))  # cat -> name -> [(id, skillname)]
for line in open(os.environ.get("OUT", "target") + "/gaps.tsv"):
    cat, name, sid, sname = line.rstrip("\n").split("\t")
    sid = int(sid)
    if sid in LIVE:
        rows[cat][name].append((sid, sname))

total = 0
for cat in ("effect", "effect-scope", "condition", "targetType", "affectScope",
            "affectObject", "operateType"):
    names = rows.get(cat, {})
    if not names:
        print(f"\n########## {cat}: NOTHING LIVE\n")
        continue
    ids = set()
    for v in names.values():
        ids |= {i for i, _ in v}
    total += len(ids)
    print(f"\n\n########## {cat} — {len(names)} live names, {len(ids)} live skills")
    for name, v in sorted(names.items(), key=lambda kv: (-len(kv[1]), kv[0])):
        print(f"\n### {name}  ({len(v)} live)")
        for sid, sname in v[:20]:
            print(f"    {sid:>6} {sname:<44} <- {', '.join(WHY[str(sid)][:3])}")
        if len(v) > 20:
            print(f"    ... +{len(v)-20} more")
print(f"\n\nTOTAL distinct live skills with a gap: {total}")
