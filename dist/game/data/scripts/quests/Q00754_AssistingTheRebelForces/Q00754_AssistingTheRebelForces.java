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
package quests.Q00754_AssistingTheRebelForces;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

/**
 * @author hlwrave
 */
public class Q00754_AssistingTheRebelForces extends Quest
{
	// Items
	private static final int REBEL_SUPPLY_BOX = 35549;
	private static final int MARK_OF_RESISTANCE = 34909;
	
	// Npcs
	private static final int SIZRAK = 33669;
	private static final int COMMUNICATION = 33676;
	
	// Monsters
	private static final int KUNDA_GUARDIAN = 23224;
	private static final int KUNDA_BERSERKER = 23225;
	private static final int KUNDA_EXECUTOR = 23226;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	private static final int KUNDA_GUARDIAN_KILL = 5;
	private static final int KUNDA_BERSERKER_KILL = 5;
	private static final int KUNDA_EXECUTOR_KILL = 5;
	
	public Q00754_AssistingTheRebelForces()
	{
		super(754);
		addStartNpc(SIZRAK);
		addTalkId(SIZRAK, COMMUNICATION);
		addKillId(KUNDA_GUARDIAN, KUNDA_BERSERKER, KUNDA_EXECUTOR);
		addCondMinLevel(MIN_LEVEL, "sofa_sizraku_q0754_05.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		if (event.equals("quest_accpted.htm"))
		{
			qs.startQuest();
			qs.set(Integer.toString(KUNDA_GUARDIAN), 0);
			qs.set(Integer.toString(KUNDA_BERSERKER), 0);
			qs.set(Integer.toString(KUNDA_EXECUTOR), 0);
			htmltext = "sofa_sizraku_q0754_04.html";
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (npc.getId())
		{
			case SIZRAK:
			{
				if (qs.isCreated())
				{
					htmltext = "sofa_sizraku_q0754_01.htm";
				}
				else if (qs.isCond(0))
				{
					htmltext = "sofa_sizraku_q0754_03.html";
				}
				else if (qs.isCond(1))
				{
					htmltext = "sofa_sizraku_q0754_07.html";
				}
				else if (qs.isCond(2))
				{
					addExpAndSp(player, 570676680, 261024840);
					giveItems(player, REBEL_SUPPLY_BOX, 1);
					giveItems(player, MARK_OF_RESISTANCE, 1);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = "sofa_sizraku_q0754_08.html";
				}
				else if (qs.isCompleted())
				{
					htmltext = "sofa_sizraku_q0754_06.html";
				}
				break;
			}
			case COMMUNICATION:
			{
				if (qs.isCond(2))
				{
					qs.getPlayer().addExpAndSp(570676680, 261024840);
					giveItems(player, REBEL_SUPPLY_BOX, 1);
					giveItems(player, MARK_OF_RESISTANCE, 1);
					qs.exitQuest(QuestType.DAILY, true);
					htmltext = "sofa_sizraku_q0754_08.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if (qs != null)
		{
			switch (npc.getId())
			{
				case KUNDA_GUARDIAN:
				{
					int kills = qs.getInt(Integer.toString(KUNDA_GUARDIAN));
					if (kills < KUNDA_GUARDIAN_KILL)
					{
						kills++;
						qs.set(Integer.toString(KUNDA_GUARDIAN), kills);
					}
					break;
				}
				case KUNDA_BERSERKER:
				{
					int kills = qs.getInt(Integer.toString(KUNDA_BERSERKER));
					if (kills < KUNDA_BERSERKER_KILL)
					{
						kills++;
						qs.set(Integer.toString(KUNDA_BERSERKER), kills);
					}
					break;
				}
				case KUNDA_EXECUTOR:
				{
					int kills = qs.getInt(Integer.toString(KUNDA_EXECUTOR));
					if (kills < KUNDA_EXECUTOR_KILL)
					{
						kills++;
						qs.set(Integer.toString(KUNDA_EXECUTOR), kills);
					}
					break;
				}
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(KUNDA_GUARDIAN, qs.getInt(Integer.toString(KUNDA_GUARDIAN)));
			log.addNpc(KUNDA_BERSERKER, qs.getInt(Integer.toString(KUNDA_BERSERKER)));
			log.addNpc(KUNDA_EXECUTOR, qs.getInt(Integer.toString(KUNDA_EXECUTOR)));
			qs.getPlayer().sendPacket(log);
			
			if ((qs.getInt(Integer.toString(KUNDA_GUARDIAN)) >= KUNDA_GUARDIAN_KILL) && (qs.getInt(Integer.toString(KUNDA_BERSERKER)) >= KUNDA_BERSERKER_KILL) && (qs.getInt(Integer.toString(KUNDA_EXECUTOR)) >= KUNDA_EXECUTOR_KILL))
			{
				qs.setCond(2);
			}
		}
	}
}
