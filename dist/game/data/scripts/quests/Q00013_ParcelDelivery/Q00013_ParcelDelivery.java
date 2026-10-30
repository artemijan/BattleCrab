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
package quests.Q00013_ParcelDelivery;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Parcel Delivery (13)
 * @author nonom
 */
public class Q00013_ParcelDelivery extends Quest
{
	// NPCs
	private static final int FUNDIN = 31274;
	private static final int VULCAN = 31539;
	
	// Item
	private static final int PACKAGE = 7263;
	
	// Misc
	private static final int MIN_LEVEL = 74;
	
	public Q00013_ParcelDelivery()
	{
		super(13);
		addStartNpc(FUNDIN);
		addTalkId(FUNDIN, VULCAN);
		registerQuestItems(PACKAGE);
		addCondMinLevel(MIN_LEVEL, "31274-01.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "31274-02.html":
			{
				qs.startQuest();
				giveItems(player, PACKAGE, 1);
				break;
			}
			case "31539-01.html":
			{
				if (qs.isCond(1) && hasQuestItems(player, PACKAGE))
				{
					giveAdena(player, 271980, true);
					addExpAndSp(player, 1_279_632, 307);
					qs.exitQuest(false, true);
				}
				else
				{
					htmltext = "31539-02.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		final int npcId = npc.getId();
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npcId == FUNDIN)
				{
					htmltext = "31274-00.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					switch (npcId)
					{
						case FUNDIN:
						{
							htmltext = "31274-02.html";
							break;
						}
						case VULCAN:
						{
							htmltext = "31539-00.html";
							break;
						}
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
