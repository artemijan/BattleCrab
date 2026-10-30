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
package ai.areas.StakatoNest;

import java.util.Collection;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.holders.npc.MinionList;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.serverpackets.MagicSkillUse;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Stakato Nest AI.
 * @author Gnacik, Mobius
 */
public class StakatoNest extends Script
{
	// Cocoons
	private static final Set<Integer> COCOONS = new HashSet<>();
	static
	{
		COCOONS.add(18793);
		COCOONS.add(18794);
		COCOONS.add(18795);
		COCOONS.add(18796);
		COCOONS.add(18797);
		COCOONS.add(18798);
	}
	
	// Cannibalistic Stakato Leader
	private static final int STAKATO_LEADER = 22625;
	
	// Spike Stakato Nurse
	private static final int STAKATO_NURSE = 22630;
	
	// Spike Stakato Nurse (Changed)
	private static final int STAKATO_NURSE_2 = 22631;
	
	// Spiked Stakato Baby
	private static final int STAKATO_BABY = 22632;
	
	// Spiked Stakato Captain
	private static final int STAKATO_CAPTAIN = 22629;
	
	// Female Spiked Stakato
	private static final int STAKATO_FEMALE = 22620;
	
	// Male Spiked Stakato
	private static final int STAKATO_MALE = 22621;
	
	// Male Spiked Stakato (Changed)
	private static final int STAKATO_MALE_2 = 22622;
	
	// Spiked Stakato Guard
	private static final int STAKATO_GUARD = 22619;
	
	// Cannibalistic Stakato Chief
	private static final int STAKATO_CHIEF = 25667;
	
	// Growth Accelerator
	private static final int GROWTH_ACCELERATOR = 2905;
	
	// Small Stakato Cocoon
	private static final int SMALL_COCOON = 14833;
	
	// Large Stakato Cocoon
	private static final int LARGE_COCOON = 14834;
	
	// Subordinate skill
	private static final SkillHolder DEVOUR_SUBORDINATE = new SkillHolder(4484, 1);
	
	private StakatoNest()
	{
		addAttackId(STAKATO_LEADER);
		addSkillSeeId(COCOONS);
		addKillId(STAKATO_NURSE, STAKATO_BABY, STAKATO_MALE, STAKATO_FEMALE, STAKATO_CHIEF);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if ((npc == null) || (player == null) || npc.isDead())
		{
			return null;
		}
		
		int npcId = 0;
		switch (event)
		{
			case "nurse_change":
			{
				npcId = STAKATO_NURSE_2;
				break;
			}
			case "male_change":
			{
				npcId = STAKATO_MALE_2;
				break;
			}
		}
		
		if (npcId > 0)
		{
			npc.getSpawn().decreaseCount(npc);
			final Npc spawned = addSpawn(npcId, npc.getX(), npc.getY(), npc.getZ(), npc.getHeading(), false, 0, true);
			addAttackPlayerDesire(spawned, player);
			npc.deleteMe();
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		if ((getRandom(100) < 10) && (npc.getCurrentHp() < (npc.getMaxHp() * 0.3)))
		{
			final Monster follower = checkMinion(npc);
			if (follower != null)
			{
				final double hp = follower.getCurrentHp();
				if (hp > (follower.getMaxHp() * 0.3))
				{
					npc.abortAttack();
					npc.abortCast();
					npc.setHeading(LocationUtil.calculateHeadingFrom(npc, follower));
					npc.doCast(DEVOUR_SUBORDINATE.getSkill());
					npc.setCurrentHp(npc.getCurrentHp() + hp);
					follower.doDie(follower);
					follower.deleteMe();
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		switch (npc.getId())
		{
			case STAKATO_NURSE:
			{
				final Monster monster = checkMinion(npc);
				if (monster != null)
				{
					npc.broadcastPacket(new MagicSkillUse(npc, 2046, 1, 1000, 0));
					for (int i = 0; i < 3; i++)
					{
						final Npc spawned = addSpawn(STAKATO_CAPTAIN, monster, true);
						addAttackPlayerDesire(spawned, killer);
					}
				}
				break;
			}
			case STAKATO_BABY:
			{
				final Monster monster = npc.asMonster().getLeader();
				if ((monster != null) && !monster.isDead())
				{
					startQuestTimer("nurse_change", 5000, monster, killer);
				}
				break;
			}
			case STAKATO_MALE:
			{
				final Monster monster = checkMinion(npc);
				if (monster != null)
				{
					npc.broadcastPacket(new MagicSkillUse(npc, 2046, 1, 1000, 0));
					for (int i = 0; i < 3; i++)
					{
						final Npc spawned = addSpawn(STAKATO_GUARD, monster, true);
						addAttackPlayerDesire(spawned, killer);
					}
				}
				break;
			}
			case STAKATO_FEMALE:
			{
				final Monster monster = npc.asMonster().getLeader();
				if ((monster != null) && !monster.isDead())
				{
					startQuestTimer("male_change", 5000, monster, killer);
				}
				break;
			}
			case STAKATO_CHIEF:
			{
				if (killer.isInParty())
				{
					final List<Player> party = killer.getParty().getMembers();
					for (Player member : party)
					{
						giveCocoon(member, npc);
					}
				}
				else
				{
					giveCocoon(killer, npc);
				}
				break;
			}
		}
	}
	
	@Override
	public void onSkillSee(Npc npc, Player caster, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		if (targets.contains(npc) && (skill.getId() == GROWTH_ACCELERATOR))
		{
			npc.doDie(caster);
			final Npc spawned = addSpawn(STAKATO_CHIEF, npc.getX(), npc.getY(), npc.getZ(), LocationUtil.calculateHeadingFrom(npc, caster), false, 0, true);
			addAttackPlayerDesire(spawned, caster);
		}
	}
	
	private static Monster checkMinion(Npc npc)
	{
		final Monster monster = npc.asMonster();
		if (monster.hasMinions())
		{
			final MinionList minionList = monster.getMinionList();
			if (minionList.getSpawnedMinionCount() > 0)
			{
				for (Monster minion : minionList.getSpawnedMinions())
				{
					if ((minion != null) && !minion.isDead())
					{
						return minion;
					}
				}
			}
		}
		
		return null;
	}
	
	private void giveCocoon(Player player, Npc npc)
	{
		player.addItem(ItemProcessType.REWARD, ((getRandom(100) > 80) ? LARGE_COCOON : SMALL_COCOON), 1, npc, true);
	}
	
	public static void main(String[] args)
	{
		new StakatoNest();
	}
}
