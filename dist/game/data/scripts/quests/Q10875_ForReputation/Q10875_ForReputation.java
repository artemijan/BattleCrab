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
package quests.Q10875_ForReputation;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10873_ExaltedReachingAnotherLevel.Q10873_ExaltedReachingAnotherLevel;

/**
 * For Reputation (10875)
 * @URL https://l2wiki.com/For_Reputation https://www.youtube.com/watch?v=7i-M4U4qxaA
 * @author Mobius
 */
public class Q10875_ForReputation extends Quest
{
	// NPC
	private static final int KRENAHT = 34237;
	private static final int KEKROPUS = 34222;
	
	// Items
	private static final int BLACKBIRD_CLAN_CERTIFICATION = 47840;
	private static final int GIANT_TRACKERS_CERTIFICATION = 47841;
	
	// Rewards
	private static final int KEKROPUS_CERTIFICATE = 47831;
	private static final int SPELLBOOK_VITALITY_OF_THE_EXALTED = 47831;
	
	// Misc
	private static final int MIN_LEVEL = 103;
	
	public Q10875_ForReputation()
	{
		super(10875);
		addStartNpc(KRENAHT);
		addTalkId(KRENAHT, KEKROPUS);
		addCondMinLevel(MIN_LEVEL, "34237-00.html");
		addCondStartedQuest(Q10873_ExaltedReachingAnotherLevel.class.getSimpleName(), "34237-00.html");
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
			case "34237-02.htm":
			case "34237-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34237-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34237-07.html":
			{
				qs.setCond(2);
				htmltext = event;
				break;
			}
			case "34222-02.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						if (!hasQuestItems(player, BLACKBIRD_CLAN_CERTIFICATION, GIANT_TRACKERS_CERTIFICATION))
						{
							htmltext = "34222-00.html";
						}
						else
						{
							htmltext = event;
							giveItems(player, KEKROPUS_CERTIFICATE, 1);
							giveItems(player, SPELLBOOK_VITALITY_OF_THE_EXALTED, 1);
							qs.exitQuest(false, true);
						}
					}
					else
					{
						htmltext = getNoQuestLevelRewardMsg(player);
					}
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
				htmltext = "34237-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case KRENAHT:
					{
						if (qs.isCond(1) && !hasQuestItems(player, BLACKBIRD_CLAN_CERTIFICATION, GIANT_TRACKERS_CERTIFICATION))
						{
							htmltext = "34237-05.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "34237-07.htm";
						}
						else
						{
							htmltext = "34237-06.htm";
						}
						break;
					}
					case KEKROPUS:
					{
						if (qs.isCond(2))
						{
							htmltext = "34222-01.htm";
						}
						else
						{
							htmltext = "34222-00.html";
						}
						break;
					}
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
}
