/*
 * Copyright (c) 2013 L2jMobius
 * 
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 * 
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 * 
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
 * WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
 * IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
package instances.KartiasLabyrinth;

import java.util.List;

import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureAttacked;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDeath;
import org.l2jmobius.gameserver.model.events.holders.instance.OnInstanceStatusChange;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Kartia Helper Adolph AI.
 * @author flanagak
 */
public class KartiaHelperAdolph extends Script
{
	// NPCs
	private static final int[] KARTIA_ADOLPH =
	{
		33609, // Adolph (Kartia 85)
		33620, // Adolph (Kartia 90)
		33631, // Adolph (Kartia 95)
	};
	private static final int[] MIRRORS =
	{
		33798, // Life Plunderer (85)
		33799, // Life Plunderer (90)
		33800, // Life Plunderer (95)
	};
	private static final int[] KARTIA_FRIENDS =
	{
		33617, // Elise (Kartia 85)
		33628, // Elise (Kartia 90)
		33639, // Elise (Kartia 95)
		33609, // Adolph (Kartia 85)
		33620, // Adolph (Kartia 90)
		33631, // Adolph (Kartia 95)
		33611, // Barton (Kartia 85)
		33622, // Barton (Kartia 90)
		33633, // Barton (Kartia 95)
		33615, // Eliyah (Kartia 85)
		33626, // Eliyah (Kartia 90)
		33637, // Eliyah (Kartia 95)
		33613, // Hayuk (Kartia 85)
		33624, // Hayuk (Kartia 90)
		33635, // Hayuk (Kartia 95)
		33618, // Eliyah's Guardian Spirit (Kartia 85)
		33629, // Eliyah's Guardian Spirit (Kartia 90)
		33640, // Eliyah's Guardian Spirit (Kartia 95)
	};
	
	// Misc
	private static final int[] KARTIA_SOLO_INSTANCES =
	{
		205, // Solo 85
		206, // Solo 90
		207, // Solo 95
	};
	
	private KartiaHelperAdolph()
	{
		addSpellFinishedId(KARTIA_ADOLPH);
		addCreatureSeeId(KARTIA_ADOLPH);
		setCreatureKillId(this::onCreatureKill, KARTIA_ADOLPH);
		setCreatureAttackedId(this::onCreatureAttacked, KARTIA_ADOLPH);
		setInstanceStatusChangeId(this::onInstanceStatusChange, KARTIA_SOLO_INSTANCES);
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		final Instance instance = npc.getInstanceWorld();
		if ((instance != null) && event.equals("CHECK_ACTION"))
		{
			final StatSet npcVars = npc.getVariables();
			final Player plr = npcVars.getObject("PLAYER_OBJECT", Player.class);
			if (plr != null)
			{
				final double distance = npc.calculateDistance2D(plr);
				if (distance > 300)
				{
					final Location loc = new Location(plr.getX(), plr.getY(), plr.getZ() + 50);
					final Location randLoc = new Location(loc.getX() + getRandom(-100, 100), loc.getY() + getRandom(-100, 100), loc.getZ());
					if (distance > 600)
					{
						npc.teleToLocation(loc);
					}
					else
					{
						npc.setRunning();
					}
					
					addMoveToDesire(npc, randLoc, 23);
				}
				else if (!npc.isInCombat() || (npc.getTarget() == null))
				{
					final List<Monster> monsterList = World.getInstance().getVisibleObjectsInRange(npc, Monster.class, 500);
					if (!monsterList.isEmpty())
					{
						final Monster monster = getRandomEntry(monsterList);
						if (monster.isTargetable() && GeoEngine.getInstance().canSeeTarget(npc, monster) && !ArrayUtil.contains(MIRRORS, monster.getId()) && !ArrayUtil.contains(KARTIA_FRIENDS, monster.getId()))
						{
							addAttackDesire(npc, monster);
						}
					}
				}
			}
		}
		else if ((instance != null) && event.equals("USE_SKILL"))
		{
			if (npc.isInCombat() || npc.isAttackingNow() || (npc.getTarget() != null))
			{
				if ((npc.getCurrentMpPercent() > 25) && !ArrayUtil.contains(KARTIA_FRIENDS, npc.getTargetId()))
				{
					useRandomSkill(npc);
				}
			}
		}
	}
	
	public void useRandomSkill(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		final WorldObject target = npc.getTarget();
		if (target == null)
		{
			return;
		}
		
		if ((instance != null) && !npc.isCastingNow() && (!ArrayUtil.contains(KARTIA_FRIENDS, target.getId())))
		{
			final StatSet instParams = instance.getTemplateParameters();
			final SkillHolder skill1 = instParams.getSkillHolder("adolphHate");
			final SkillHolder skill2 = instParams.getSkillHolder("adolphPunish");
			final SkillHolder skill3 = instParams.getSkillHolder("adolphShield");
			final SkillHolder skill4 = instParams.getSkillHolder("adolphAggression");
			final SkillHolder skill5 = instParams.getSkillHolder("adolphChainHydra");
			final SkillHolder lowHpSkill = instParams.getSkillHolder("adolphUltimate");
			final int numberOfActiveSkills = 5;
			final int randomSkill = getRandom(numberOfActiveSkills + 1);
			if ((lowHpSkill != null) && (npc.getCurrentHpPercent() < 50))
			{
				// NPC has low HP Skill and HP is < 50% HP
				if (!npc.isAffectedBySkill(lowHpSkill.getSkillId()) && (SkillCaster.checkUseConditions(npc, lowHpSkill.getSkill())))
				{
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.IT_S_NOT_OVER);
					npc.doCast(lowHpSkill.getSkill());
				}
			}
			else
			{
				switch (randomSkill)
				{
					case 0:
					case 1:
					{
						if ((skill1 != null) && SkillCaster.checkUseConditions(npc, skill1.getSkill()))
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_WILL_SHOW_YOU_THE_JUSTICE_OF_ADEN);
							npc.doCast(skill1.getSkill(), null, true, false);
							final List<Monster> monsterList = World.getInstance().getVisibleObjectsInRange(npc, Monster.class, 300);
							if (!monsterList.isEmpty())
							{
								for (Monster monster : monsterList)
								{
									if (monster.isTargetable() && GeoEngine.getInstance().canSeeTarget(npc, monster) && !ArrayUtil.contains(MIRRORS, monster.getId()) && !ArrayUtil.contains(KARTIA_FRIENDS, monster.getId()))
									{
										monster.addDamageHate(npc, 100, 10000);
									}
								}
							}
						}
						break;
					}
					case 2:
					{
						if ((skill2 != null) && SkillCaster.checkUseConditions(npc, skill2.getSkill()))
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DIE_3);
							npc.doCast(skill2.getSkill(), null, true, false);
						}
						break;
					}
					case 3:
					{
						if ((skill3 != null) && SkillCaster.checkUseConditions(npc, skill3.getSkill()))
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.FOR_THE_GODDESS);
							npc.doCast(skill3.getSkill(), null, true, false);
						}
						break;
					}
					case 4:
					{
						if ((skill4 != null) && SkillCaster.checkUseConditions(npc, skill4.getSkill()))
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_WILL_BE_DESTROYED);
							npc.doCast(skill4.getSkill());
						}
						break;
					}
					case 5:
					{
						if ((skill5 != null) && SkillCaster.checkUseConditions(npc, skill5.getSkill()))
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_WILL_DIE);
							npc.doCast(skill5.getSkill(), null, true, false);
						}
						break;
					}
				}
			}
		}
	}
	
	public void onCreatureAttacked(OnCreatureAttacked event)
	{
		final Npc npc = event.getTarget().asNpc();
		if (npc != null)
		{
			final Instance instance = npc.getInstanceWorld();
			if ((instance != null) && !event.getAttacker().isPlayable() && !ArrayUtil.contains(KARTIA_FRIENDS, event.getAttacker().getId()))
			{
				if (!npc.isInCombat())
				{
					npc.setTarget(event.getAttacker());
					addAttackDesire(npc, npc.getTarget().asCreature());
					final StatSet instParams = instance.getTemplateParameters();
					final SkillHolder hateSkill = instParams.getSkillHolder("adolphHate");
					if ((hateSkill != null) && SkillCaster.checkUseConditions(npc, hateSkill.getSkill()))
					{
						npc.doCast(hateSkill.getSkill(), null, true, false);
					}
				}
			}
		}
	}
	
	public void onCreatureKill(OnCreatureDeath event)
	{
		final Npc npc = event.getTarget().asNpc();
		final Instance world = npc.getInstanceWorld();
		if (world != null)
		{
			getTimers().cancelTimersOf(npc);
			world.destroy(); // Kartia can't continue without Adolph
		}
	}
	
	public void onInstanceStatusChange(OnInstanceStatusChange event)
	{
		final Instance instance = event.getWorld();
		final int status = event.getStatus();
		if (status == 1)
		{
			instance.getAliveNpcs(KARTIA_ADOLPH).forEach(adolph -> getTimers().addRepeatingTimer("CHECK_ACTION", 3000, adolph, null));
			instance.getAliveNpcs(KARTIA_ADOLPH).forEach(adolph -> getTimers().addRepeatingTimer("USE_SKILL", 6000, adolph, null));
		}
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			npc.getVariables().set("PLAYER_OBJECT", creature.asPlayer());
		}
	}
	
	public static void main(String[] args)
	{
		new KartiaHelperAdolph();
	}
}
