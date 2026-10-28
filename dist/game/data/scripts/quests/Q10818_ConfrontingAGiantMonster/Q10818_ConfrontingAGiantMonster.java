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
package quests.Q10818_ConfrontingAGiantMonster;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10817_ExaltedOneWhoOvercomesTheLimit.Q10817_ExaltedOneWhoOvercomesTheLimit;

/**
 * Confronting a Giant Monster (10818)
 * @URL https://l2wiki.com/Confronting_a_Giant_Monster
 * @author Mobius
 */
public class Q10818_ConfrontingAGiantMonster extends Quest
{
	// NPC
	private static final int DAICHIR = 30537;
	
	// Monsters
	private static final int ISTINA = 29196; // Extreme
	private static final int OCTAVIS = 29212; // Extreme
	private static final int TAUTI = 29233; // correct id?
	private static final int EKIMUS = 29251; // correct id?
	private static final int TRASKEN = 29197; // correct id?
	
	// Items
	private static final int DARK_SOUL_STONE = 46055;
	private static final int OLYMPIAD_MANAGER_CERTIFICATE = 45629;
	private static final int ISHUMA_CERTIFICATE = 45630;
	private static final int SIR_KRISTOF_RODEMAI_CERTIFICATE = 45631;
	
	// Rewards
	private static final int EXP_AMOUNT = 542310795;
	private static final int DAICHIR_SERTIFICATE = 45628;
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10818_ConfrontingAGiantMonster()
	{
		super(10818);
		addStartNpc(DAICHIR);
		addTalkId(DAICHIR);
		addKillId(ISTINA, OCTAVIS, TAUTI, EKIMUS, TRASKEN);
		addCondMinLevel(MIN_LEVEL, "30537-02.html");
		addCondStartedQuest(Q10817_ExaltedOneWhoOvercomesTheLimit.class.getSimpleName(), "30537-03.html");
		registerQuestItems(DARK_SOUL_STONE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "30537-04.htm":
			case "30537-05.htm":
			case "30537-06.htm":
			{
				htmltext = event;
				break;
			}
			case "30537-06a.html":
			{
				if (qs.isCreated())
				{
					qs.startQuest();
				}
				
				htmltext = event;
				break;
			}
			case "30537-09.html":
			{
				if ((player.getLevel() >= MIN_LEVEL))
				{
					if (hasQuestItems(player, DARK_SOUL_STONE) && qs.get(Integer.toString(ISTINA)).equals("true") && qs.get(Integer.toString(OCTAVIS)).equals("true") && qs.get(Integer.toString(TAUTI)).equals("true") && qs.get(Integer.toString(EKIMUS)).equals("true"))
					{
						if (hasQuestItems(player, OLYMPIAD_MANAGER_CERTIFICATE, ISHUMA_CERTIFICATE, SIR_KRISTOF_RODEMAI_CERTIFICATE))
						{
							htmltext = "30537-10.html";
						}
						else
						{
							htmltext = event;
						}
						
						takeItems(player, DARK_SOUL_STONE, -1);
						giveItems(player, DAICHIR_SERTIFICATE, 1);
						addExpAndSp(player, EXP_AMOUNT, 0);
						qs.unset(Integer.toString(ISTINA));
						qs.unset(Integer.toString(OCTAVIS));
						qs.unset(Integer.toString(TAUTI));
						qs.unset(Integer.toString(EKIMUS));
						qs.exitQuest(false, true);
					}
				}
				else
				{
					htmltext = getNoQuestLevelRewardMsg(player);
				}
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
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "30537-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (hasQuestItems(player, DARK_SOUL_STONE) && qs.get(Integer.toString(ISTINA)).equals("true") && qs.get(Integer.toString(OCTAVIS)).equals("true") && qs.get(Integer.toString(TAUTI)).equals("true") && qs.get(Integer.toString(EKIMUS)).equals("true"))
				{
					htmltext = "30537-08.html";
				}
				else
				{
					htmltext = "30537-07.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isStarted() && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			if (npc.getId() == TRASKEN)
			{
				giveItems(player, DARK_SOUL_STONE, 1);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			else
			{
				qs.set(Integer.toString(npc.getId()), "true");
			}
		}
	}
}
