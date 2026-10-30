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
package quests.Q10857_SecretTeleport;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10856_SuperionAppears.Q10856_SuperionAppears;

/**
 * Secret Teleport (10857)
 * @URL https://l2wiki.com/Secret_Teleport
 * @author Dmitri
 */
public class Q10857_SecretTeleport extends Quest
{
	// NPCs
	private static final int KEKROPUS = 34222;
	private static final int HISTIE = 34243;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23774, // Delta Bathus
		23775, // Delta Carcass
		23776, // Delta Kshana
		23777, // Royal Templar
		23778, // Royal Shooter
		23779, // Royal Wizard
		23780, // Royal Templar Colonel
		23781, // Royal Sharpshooter
		23782, // Royal Archmage
		23783 // Royal Gatekeeper
	};
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	// Items
	private static final int SUPERION_MAP_PIECE = 47191; // Quest item: Old Box
	
	// Reward
	private static final int GIANTS_ENERGY = 35563;
	
	public Q10857_SecretTeleport()
	{
		super(10857);
		addStartNpc(KEKROPUS);
		addTalkId(KEKROPUS, HISTIE);
		addKillId(MONSTERS);
		registerQuestItems(SUPERION_MAP_PIECE);
		addCondMinLevel(MIN_LEVEL, "34222-00.htm");
		addFactionLevel(Faction.GIANT_TRACKERS, 3, "34222-00.htm");
		addCondCompletedQuest(Q10856_SuperionAppears.class.getSimpleName(), "34222-00.htm");
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
			case "34222-03.htm":
			case "34222-02.htm":
			case "34222-05.htm":
			case "34243-02.htm":
			{
				htmltext = event;
				break;
			}
			case "34222-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34243-03.htm":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "teleport":
			{
				player.teleToLocation(79827, 152588, 2309);
				break;
			}
			case "34243-05.html":
			{
				if (qs.isCond(3))
				{
					addExpAndSp(player, 17777142360L, 42664860);
					giveItems(player, GIANTS_ENERGY, 1);
					qs.exitQuest(false, true);
					htmltext = event;
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
				if (npc.getId() == KEKROPUS)
				{
					htmltext = "34222-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case KEKROPUS:
					{
						if (qs.getCond() == 1)
						{
							htmltext = "34222-04.htm";
						}
						break;
					}
					case HISTIE:
					{
						if (qs.getCond() == 1)
						{
							htmltext = "34243-01.htm";
						}
						else if (qs.getCond() == 2)
						{
							htmltext = "34243-03.htm";
						}
						else if (qs.getCond() == 3)
						{
							htmltext = "34243-04.html";
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					htmltext = "34222-01.htm";
					break;
				}
				
				qs.setState(State.CREATED);
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, -1, 3, npc);
		if ((qs != null) && qs.isCond(2) && giveItemRandomly(killer, SUPERION_MAP_PIECE, 1, 20, 0.9, true))
		{
			qs.setCond(3, true);
		}
	}
}
