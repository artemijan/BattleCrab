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
package quests.Q00753_ReactingToACrisis;

import java.util.Collection;

import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10386_MysteriousJourney.Q10386_MysteriousJourney;

/**
 * Uncover the Secret (753)
 * @URL https://l2wiki.com/Reacting_to_a_Crisis
 * @VIDEO http://www.dailymotion.com/video/x24y4lx_quest-reacting-to-a-crisis_videogames
 * @author Gigi
 */
public class Q00753_ReactingToACrisis extends Quest
{
	// Npc
	private static final int BERNA = 33796;
	
	// Monster's
	private static final int GOLEM_GENERATOR = 19296;
	private static final int BATTLE_GOLEM = 23269;
	private static final int[] MOBS =
	{
		23270, // Patrol Fighter
		23271, // Patrol Archer
		23272, // Spicula Fighter
		23273, // Spicula Archer
		23274, // Specula Elite Captain
		23275, // Spicula Captain
		23276 // Cheif Scout
	};
	
	// Items
	private static final int RED_GATE_KEY = 36054;
	private static final int VERNAS_VACCINE = 36065;
	private static final int SCROLL = 36082;
	
	// Skills;
	private static final int VACCINE = 9584;
	private static final double DAMAGE_BY_SKILL = 0.5d; // Percent
	
	// Misc
	private static final int MIN_LEVEL = 93;
	
	public Q00753_ReactingToACrisis()
	{
		super(753);
		addStartNpc(BERNA);
		addTalkId(BERNA);
		addKillId(GOLEM_GENERATOR);
		addKillId(MOBS);
		addSkillSeeId(GOLEM_GENERATOR);
		registerQuestItems(RED_GATE_KEY, VERNAS_VACCINE);
		addCondMinLevel(MIN_LEVEL, "lvl.htm");
		addCondCompletedQuest(Q10386_MysteriousJourney.class.getSimpleName(), "restriction.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "33796-02.htm":
			{
				qs.startQuest();
				giveItems(player, VERNAS_VACCINE, 1);
				htmltext = event;
				break;
			}
			case "33796-05.html":
			{
				giveItems(player, SCROLL, 1);
				addExpAndSp(player, 408665250, 98079);
				qs.exitQuest(QuestType.DAILY, true);
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (npc.getId() == BERNA)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "33796-00.htm";
						break;
					}
					
					qs.setState(State.CREATED);
					// fallthrough
				}
				case State.CREATED:
				{
					htmltext = "33796-01.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "33796-03.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "33796-04.html";
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onSkillSee(Npc npc, Player player, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		if (!npc.isDead() && (player.getTarget() == npc) && (skill.getId() == VACCINE))
		{
			final double dmg = npc.getMaxHp() * DAMAGE_BY_SKILL;
			npc.reduceCurrentHp(dmg, player, null);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if (ArrayUtil.contains(MOBS, npc.getId()) && (qs != null) && qs.isCond(1) && (giveItemRandomly(killer, RED_GATE_KEY, 1, 30, 0.2, true)))
		{
			qs.setMemoState(1);
		}
		
		if (((npc.getId() == 23275) || (npc.getId() == 23276) || (npc.getId() == 23274)) && (getRandom(100) < 10))
		{
			addSpawn(GOLEM_GENERATOR, npc.getX() + 30, npc.getY() + 30, npc.getZ(), 0, false, 60000);
			showOnScreenMsg(killer, NpcStringId.THE_GOLEM_GENERATOR_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 6000);
		}
		
		if ((qs != null) && qs.isCond(1) && (npc.getId() == GOLEM_GENERATOR))
		{
			int kills = qs.getInt(Integer.toString(GOLEM_GENERATOR));
			if (kills < 5)
			{
				kills++;
				qs.set(Integer.toString(GOLEM_GENERATOR), kills);
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpcString(NpcStringId.USE_VACCINE_ON_GOLEM_GENERATOR, kills);
			killer.sendPacket(log);
			for (int i = 0; i < 4; i++)
			{
				final Npc mob = addSpawn(BATTLE_GOLEM, killer, true, 70000);
				addAttackPlayerDesire(mob, killer);
			}
		}
		
		if ((qs != null) && (qs.getInt(Integer.toString(GOLEM_GENERATOR)) >= 5) && (qs.isMemoState(1)))
		{
			takeItems(killer, VERNAS_VACCINE, -1);
			qs.setCond(2, true);
		}
	}
}
