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
package quests.Q00621_EggDelivery;

import java.util.Arrays;
import java.util.List;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Egg Delivery (621)
 * @author Janiko
 */
public class Q00621_EggDelivery extends Quest
{
	// NPCs
	private static final int JEREMY = 31521;
	private static final int PULIN = 31543;
	private static final int NAFF = 31544;
	private static final int CROCUS = 31545;
	private static final int KUBER = 31546;
	private static final int BOELIN = 31547;
	private static final int VALENTINE = 31584;
	
	// Items
	private static final int BOILED_EGG = 7195;
	private static final int EGG_PRICE = 7196;
	
	// Misc
	private static final int MIN_LEVEL = 68;
	
	// Reward
	private static final int QUICK_STEP_POTION = 734;
	private static final int SEALED_RING_OF_AURAKYRA = 6849;
	private static final int SEALED_SANDDRAGONS_EARING = 6847;
	private static final int SEALED_DRAGON_NECKLACE = 6851;
	
	// Talkers
	private static final List<Integer> TALKERS = Arrays.asList(NAFF, CROCUS, KUBER, BOELIN);
	
	public Q00621_EggDelivery()
	{
		super(621);
		addStartNpc(JEREMY);
		addTalkId(JEREMY, PULIN, VALENTINE);
		addTalkId(TALKERS);
		registerQuestItems(BOILED_EGG, EGG_PRICE);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		String htmltext = null;
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "31521-03.htm":
			{
				if (qs.isCreated())
				{
					qs.startQuest();
					giveItems(player, BOILED_EGG, 5);
					htmltext = event;
				}
				break;
			}
			case "31521-06.html":
			{
				if (qs.isCond(6))
				{
					if (getQuestItemsCount(player, EGG_PRICE) >= 5)
					{
						qs.setCond(7, true);
						takeItems(player, EGG_PRICE, -1);
						htmltext = event;
					}
					else
					{
						htmltext = "31521-07.html";
					}
				}
				break;
			}
			case "31543-02.html":
			{
				if (qs.isCond(1))
				{
					if (hasQuestItems(player, BOILED_EGG))
					{
						qs.setCond(2, true);
						takeItems(player, BOILED_EGG, 1);
						giveItems(player, EGG_PRICE, 1);
						htmltext = event;
					}
					else
					{
						htmltext = "31543-03.html";
					}
				}
				break;
			}
			case "31544-02.html":
			case "31545-02.html":
			case "31546-02.html":
			case "31547-02.html":
			{
				if (TALKERS.contains(npc.getId()) && qs.isCond(TALKERS.indexOf(npc.getId()) + 2))
				{
					if (hasQuestItems(player, BOILED_EGG))
					{
						qs.setCond(qs.getCond() + 1, true);
						takeItems(player, BOILED_EGG, 1);
						giveItems(player, EGG_PRICE, 1);
						htmltext = event;
					}
					else
					{
						htmltext = npc.getId() + "-03.html";
					}
				}
				break;
			}
			case "31584-02.html":
			{
				if (qs.isCond(7))
				{
					final int rnd = getRandom(1000);
					if (rnd < 800)
					{
						rewardItems(player, QUICK_STEP_POTION, 1);
						giveAdena(player, 18800, true);
					}
					else if (rnd < 880)
					{
						rewardItems(player, SEALED_RING_OF_AURAKYRA, 1);
					}
					else if (rnd < 960)
					{
						rewardItems(player, SEALED_SANDDRAGONS_EARING, 1);
					}
					else if (rnd < 1000)
					{
						rewardItems(player, SEALED_DRAGON_NECKLACE, 1);
					}
					
					qs.exitQuest(true, true);
					htmltext = event;
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		final QuestState qs = getQuestState(talker, true);
		String htmltext = getNoQuestMsg(talker);
		switch (npc.getId())
		{
			case JEREMY:
			{
				switch (qs.getState())
				{
					case State.CREATED:
					{
						htmltext = (talker.getLevel() >= MIN_LEVEL) ? "31521-01.htm" : "31521-02.htm";
						break;
					}
					case State.STARTED:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "31521-04.html";
								break;
							}
							case 6:
							{
								if (hasQuestItems(talker, EGG_PRICE))
								{
									htmltext = "31521-05.html";
								}
								break;
							}
							case 7:
							{
								if (!hasQuestItems(talker, BOILED_EGG))
								{
									htmltext = "31521-08.html";
								}
								break;
							}
						}
						break;
					}
					case State.COMPLETED:
					{
						htmltext = getAlreadyCompletedMsg(talker);
						break;
					}
				}
				break;
			}
			case PULIN:
			{
				if (qs.isStarted())
				{
					switch (qs.getCond())
					{
						case 1:
						{
							if (getQuestItemsCount(talker, BOILED_EGG) >= 5)
							{
								htmltext = "31543-01.html";
							}
							break;
						}
						case 2:
						{
							htmltext = "31543-04.html";
							break;
						}
					}
				}
				break;
			}
			case NAFF:
			case CROCUS:
			case KUBER:
			case BOELIN:
			{
				if (qs.isStarted())
				{
					final int cond = TALKERS.indexOf(npc.getId()) + 2;
					if (qs.isCond(cond) && hasQuestItems(talker, EGG_PRICE)) // 2,3,4,5
					{
						htmltext = npc.getId() + "-01.html";
					}
					else if (qs.isCond(cond + 1)) // 3,4,5,6
					{
						htmltext = npc.getId() + "-04.html";
					}
				}
				break;
			}
			case VALENTINE:
			{
				if (qs.isStarted() && qs.isCond(7))
				{
					htmltext = "31584-01.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
