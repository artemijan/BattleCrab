#!/usr/bin/env python3
"""Second-order census filter: of the skills the parser drops, which ones can a
player on THIS server actually meet?

reachable (census)  = referenced anywhere in the datapack XML
live      (here)    = learnable, OR on a pet, OR on an NPC that is spawned,
                      OR on an item that is obtainable (drop from a spawned
                      npc / buylist / multisell / recipe / quest reward).
"""
import re, sys, os, json
from collections import defaultdict

DIST = os.environ.get("L2J_DIST", "../interlude_classic/dist/game/")
D = DIST + "data/"

def walk(sub, ext=".xml"):
    for root, _, files in os.walk(D + sub):
        for f in files:
            if f.endswith(ext):
                yield os.path.join(root, f)

def read(p):
    try:
        return open(p, encoding="utf-8", errors="replace").read()
    except OSError:
        return ""

# ---------------------------------------------------------------- spawned npcs
spawned = set()
for p in walk("spawns"):
    spawned.update(int(m) for m in re.findall(r'<npc id="(\d+)"', read(p)))
for p in walk("instances"):
    spawned.update(int(m) for m in re.findall(r'<npc id="(\d+)"', read(p)))
# script-driven spawns: addSpawn(12345, ...) / SPAWN = 12345 style constants
script_txt = []
for root, _, files in os.walk(D + "scripts"):
    for f in files:
        if f.endswith((".java", ".xml")):
            script_txt.append(read(os.path.join(root, f)))
script_blob = "\n".join(script_txt)
spawned.update(int(m) for m in re.findall(r'addSpawn\(\s*(\d{4,5})', script_blob))

# ------------------------------------------------------- npc blocks: skills+drops+minions
npc_skills = defaultdict(set)   # npc id -> skill ids
npc_drops  = defaultdict(set)   # npc id -> item ids
minions    = defaultdict(set)
npc_name   = {}
NPC_BLOCK = re.compile(r'<npc id="(\d+)"[^>]*name="([^"]*)"(.*?)</npc>', re.S)
for p in walk("stats/npcs"):
    for nid, nm, body in NPC_BLOCK.findall(read(p)):
        nid = int(nid); npc_name[nid] = nm
        npc_skills[nid].update(int(x) for x in re.findall(r'<skill id="(\d+)"', body))
        npc_drops[nid].update(int(x) for x in re.findall(r'<item id="(\d+)"', body))
        minions[nid].update(int(x) for x in re.findall(r'<minion id="(\d+)"', body))

# minions of spawned npcs are spawned too (transitively)
frontier = set(spawned)
while frontier:
    nxt = set()
    for n in frontier:
        for m in minions.get(n, ()):
            if m not in spawned:
                spawned.add(m); nxt.add(m)
    frontier = nxt

# ------------------------------------------------------------- obtainable items
obtainable = set()
for n in spawned:
    obtainable |= npc_drops.get(n, set())
for p in walk("buylists"):
    obtainable.update(int(x) for x in re.findall(r'<item id="(\d+)"', read(p)))
for p in walk("multisell"):
    t = read(p)
    obtainable.update(int(x) for x in re.findall(r'<production id="(\d+)"', t))
    obtainable.update(int(x) for x in re.findall(r'<ingredient id="(\d+)"', t))
for f in ("Recipes.xml", "Fishing.xml", "CombinationItems.xml", "CrystallizableItems.xml",
          "LuckyGameData.xml", "PrimeShop.xml", "AttendanceRewards.xml", "DailyMission.xml",
          "EnchantItemData.xml", "AppearanceStones.xml", "ItemAuctions.xml"):
    t = read(D + f)
    obtainable.update(int(x) for x in re.findall(r'(?:item|production|reward)[^>]*?id="(\d+)"', t))
    obtainable.update(int(x) for x in re.findall(r'itemId="(\d+)"', t))
# quest / script rewards
for pat in (r'giveItems\(\s*\w+\s*,\s*(\d{2,5})', r'rewardItems\(\s*\w+\s*,\s*(\d{2,5})',
            r'addItem\(\s*[^,]+,\s*(\d{2,5})', r'giveItemRandomly\([^,]+,[^,]*,\s*(\d{2,5})'):
    obtainable.update(int(m) for m in re.findall(pat, script_blob))
# character-creation / newbie gear
for p in walk("stats"):
    if "initial" in os.path.basename(p).lower():
        obtainable.update(int(x) for x in re.findall(r'id="(\d+)"', read(p)))

# --------------------------------------------------------------- item -> skills
item_skills = defaultdict(set)
item_name = {}
ITEM_BLOCK = re.compile(r'<item id="(\d+)"[^>]*name="([^"]*)"(.*?)</item>', re.S)
for p in walk("stats/items"):
    for iid, nm, body in ITEM_BLOCK.findall(read(p)):
        iid = int(iid); item_name[iid] = nm
        s = set(int(x) for x in re.findall(r'<skill id="(\d+)"', body))
        s |= set(int(x) for x in re.findall(r'skillId="(\d+)"', body))
        # enchant4_skill / item skill "id-level" val forms
        for v in re.findall(r'val="(\d+)-\d+"', body):
            s.add(int(v))
        item_skills[iid] |= s

# ------------------------------------------------------------------- learnable
learnable = set()
for p in walk("skillTrees"):
    learnable.update(int(x) for x in re.findall(r'skillId="(\d+)"', read(p)))
pets = set(int(x) for x in re.findall(r'skillId="(\d+)"', read(D + "PetSkillData.xml")))

# ------------------------------------------------------- live skills + why
why = defaultdict(list)
for s in learnable: why[s].append("LEARN")
for s in pets: why[s].append("PET")
for n in spawned:
    for s in npc_skills.get(n, ()):
        if len(why[s]) < 6:
            why[s].append(f"npc:{npc_name.get(n,n)}({n})")
for i in obtainable:
    for s in item_skills.get(i, ()):
        if len(why[s]) < 6:
            why[s].append(f"item:{item_name.get(i,i)}({i})")
live = set(why)

json.dump({"live": sorted(live),
           "why": {str(k): v for k, v in why.items()}},
          open(os.environ.get("OUT", "target") + "/live.json", "w"))
print(f"spawned npcs      : {len(spawned)}")
print(f"obtainable items  : {len(obtainable)}")
print(f"learnable skills  : {len(learnable)}")
print(f"LIVE skills total : {len(live)}")
