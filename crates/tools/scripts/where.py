#!/usr/bin/env python3
"""Where does item X come from? drop / buylist / multisell / script."""
import re, os, sys
from collections import defaultdict

D = os.environ.get("L2J_DIST", "../interlude_classic/dist/game/") + "data/"
TARGETS = set(int(x) for x in sys.argv[1:])
found = defaultdict(list)

def walk(sub, ext=(".xml",)):
    for root, _, files in os.walk(D + sub):
        for f in files:
            if f.endswith(ext):
                yield os.path.join(root, f)

def read(p):
    return open(p, encoding="utf-8", errors="replace").read()

spawned = set()
for p in list(walk("spawns")) + list(walk("instances")):
    spawned.update(int(m) for m in re.findall(r'<npc id="(\d+)"', read(p)))

NPC_BLOCK = re.compile(r'<npc id="(\d+)"[^>]*name="([^"]*)"(.*?)</npc>', re.S)
for p in walk("stats/npcs"):
    for nid, nm, body in NPC_BLOCK.findall(read(p)):
        nid = int(nid)
        for iid in re.findall(r'<item id="(\d+)"', body):
            if int(iid) in TARGETS:
                tag = "DROP(spawned)" if nid in spawned else "drop(NOT spawned)"
                found[int(iid)].append(f"{tag} {nm}({nid})")

for p in walk("buylists"):
    for iid in re.findall(r'<item id="(\d+)"', read(p)):
        if int(iid) in TARGETS:
            found[int(iid)].append(f"BUYLIST {os.path.basename(p)}")

for p in walk("multisell"):
    t = read(p)
    for iid in re.findall(r'<production id="(\d+)"', t):
        if int(iid) in TARGETS:
            found[int(iid)].append(f"MULTISELL-prod {os.path.basename(p)}")
    for iid in re.findall(r'<ingredient id="(\d+)"', t):
        if int(iid) in TARGETS:
            found[int(iid)].append(f"multisell-ingr {os.path.basename(p)}")

for root, _, files in os.walk(D + "scripts"):
    for f in files:
        if not f.endswith((".java", ".xml")):
            continue
        p = os.path.join(root, f)
        t = read(p)
        for m in re.finditer(r'(giveItems|rewardItems|addItem|giveItemRandomly)\([^)]*?\b(\d{3,5})\b', t):
            if int(m.group(2)) in TARGETS:
                found[int(m.group(2))].append(f"SCRIPT {m.group(1)} {os.path.relpath(p, D)}")

for f in ("Recipes.xml", "Fishing.xml", "CombinationItems.xml", "LuckyGameData.xml",
          "PrimeShop.xml", "AttendanceRewards.xml", "DailyMission.xml", "EnchantItemData.xml"):
    try:
        t = read(D + f)
    except OSError:
        continue
    for iid in re.findall(r'id="(\d+)"', t):
        if int(iid) in TARGETS:
            found[int(iid)].append(f"FILE {f}")

for t in sorted(TARGETS):
    src = found.get(t, [])
    uniq = []
    for s in src:
        if s not in uniq:
            uniq.append(s)
    print(f"\n{t}: {len(src)} refs")
    for s in uniq[:6]:
        print(f"    {s}")
    if not src:
        print("    *** NOT OBTAINABLE ***")
