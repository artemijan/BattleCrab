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
package quests.Q10376_BloodyGoodTime;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

import quests.Q10375_SuccubusDisciples.Q10375_SuccubusDisciples;

/**
 * Bloody Good Time (10376)
 * @URL https://l2wiki.com/Bloody_Good_Time
 * @author Gigi
 */
public class Q10376_BloodyGoodTime extends Quest
{
	// NPCs
	private static final int ZENYA = 32140;
	private static final int CASCA = 32139;
	private static final int AGNES = 31588;
	private static final int ANDREI = 31292;
	private static final int MOB_BLOODY_VEIN = 27481;
	
	// Misc
	private static final int MIN_LEVEL = 80;
	
	// Reward
	private static final int EXP_REWARD = 121297500;
	private static final int SP_REWARD = 29111;
	
	// Items
	private static final ItemHolder REWARD_MAGIC_RUNE_CLIP = new ItemHolder(32700, 1);
	
	// Location
	private static final Location RETURN_LOC = new Location(178648, -84903, -7216);
	
	public Q10376_BloodyGoodTime()
	{
		super(10376);
		addStartNpc(ZENYA);
		addTalkId(ZENYA, CASCA, AGNES, ANDREI);
		addFirstTalkId(CASCA);
		addKillId(MOB_BLOODY_VEIN);
		addCondMinLevel(MIN_LEVEL, "noLevel.html");
		addCondCompletedQuest(Q10375_SuccubusDisciples.class.getSimpleName(), "restriction.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "32139-05.html":
			case "32139-07.html":
			case "32139-08.html":
			case "32139-09.html":
			case "31588-02.html":
			case "32140-02.htm":
			case "32140-07.html":
			case "31292-02.htm":
			{
				htmltext = event;
				break;
			}
			case "32140-06.html":
			{
				qs.startQuest();
				htmltext = "32140-06.html";
				break;
			}
			case "32139-02.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
					htmltext = event;
					break;
				}
			}
			case "return":
			{
				if (player.isInCombat())
				{
					player.sendPacket(new ExShowScreenMessage("You cannot teleport when you in combat status.", 5000));
				}
				else
				{
					qs.setCond(2);
					player.teleToLocation(RETURN_LOC, 0);
					player.setInstance(null);
				}
			}
			case "32139-03.html":
			{
				htmltext = event;
				break;
			}
			case "31588-03.html":
			{
				qs.setCond(6, true);
				break;
			}
			case "31292-03.html":
			{
				addExpAndSp(player, EXP_REWARD, SP_REWARD);
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), NpcStringId.WELL_DONE_I_WAS_RIGHT_TO_ENTRUST_THIS_TO_YOU));
				giveItems(player, REWARD_MAGIC_RUNE_CLIP);
				htmltext = event;
				qs.exitQuest(false, true);
			}
				break;
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (qs.isCompleted())
		{
			htmltext = getAlreadyCompletedMsg(player);
		}
		
		switch (npc.getId())
		{
			case ZENYA:
			{
				switch (qs.getState())
				{
					case State.CREATED:
					{
						if (player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
						{
							return "32140-01.htm";
						}
						
						return "32140-03.html";
					}
					case State.STARTED:
					{
						return "32140-07.html";
					}
					case State.COMPLETED:
					{
						return "32140-05.htm";
					}
				}
				break;
			}
			case CASCA:
			{
				if (qs.isStarted())
				{
					if (qs.isCond(1))
					{
						return "32139-01.html";
					}
					else if (qs.isCond(2))
					{
						return "32139-03.html";
					}
					else if (qs.isCond(3))
					{
						return "32139-04.html";
					}
					else if (qs.isCond(4))
					{
						return "32139-06.html";
					}
				}
				break;
			}
			case AGNES:
			{
				if (qs.isStarted())
				{
					if (qs.isCond(5))
					{
						htmltext = "31588-01.html";
					}
					else if (qs.isCond(6))
					{
						htmltext = "31588-03.html";
					}
				}
				break;
			}
			case ANDREI:
			{
				if (qs.isStarted() && qs.isCond(6))
				{
					htmltext = "31292-01.htm";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "32139.html";
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(3))
		{
			qs.setCond(4, true);
		}
	}
}
