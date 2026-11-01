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
package quests.Q00465_WeAreFriends;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * We Are Friends (465)
 * @URL https://l2wiki.com/We_Are_Friends
 * @author Gigi
 */
public class Q00465_WeAreFriends extends Quest
{
	// NPCs
	private static final int FAIRY_CITIZEN = 32921;
	private static final int FAIRY_CITIZEN_SPAWN = 32923;
	
	// Item
	private static final int MARK_OF_FRIENDSHIP = 17377;
	private static final int FAIRY_LEAF_FLUTE = 17378;
	private static final int CERTIFICATE_OF_PROMISE = 30384;
	
	// Misc
	private static final int MIN_LEVEL = 88;
	
	public Q00465_WeAreFriends()
	{
		super(465);
		addStartNpc(FAIRY_CITIZEN);
		addTalkId(FAIRY_CITIZEN, FAIRY_CITIZEN_SPAWN);
		registerQuestItems(MARK_OF_FRIENDSHIP);
		addCondMinLevel(MIN_LEVEL, "no_level.htm");
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
			case "32921-02.htm":
			case "32921-07.html":
			{
				htmltext = event;
				break;
			}
			case "32921-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32923-02.html":
			{
				giveItems(player, MARK_OF_FRIENDSHIP, 1);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				if ((getQuestItemsCount(player, MARK_OF_FRIENDSHIP) >= 2))
				{
					qs.setCond(2, true);
				}
				
				htmltext = event;
				npc.deleteMe();
				break;
			}
			case "32921-08.html":
			{
				giveItems(player, FAIRY_LEAF_FLUTE, 1);
				giveItems(player, CERTIFICATE_OF_PROMISE, getRandom(1, 4));
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
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable() && (npc.getId() == FAIRY_CITIZEN))
				{
					htmltext = "32921-04.html";
					break;
				}
				
				qs.setState(State.CREATED);
				// fallthrough
			}
			case State.CREATED:
			{
				if (npc.getId() == FAIRY_CITIZEN)
				{
					htmltext = "32921-01.htm";
				}
			}
				break;
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case FAIRY_CITIZEN:
					{
						if (qs.isCond(1))
						{
							htmltext = "32921-05.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "32921-06.html";
						}
						break;
					}
					case FAIRY_CITIZEN_SPAWN:
					{
						if (qs.isCond(1) && npc.getTitle().equals(player.getName()))
						{
							htmltext = "32923-01.html";
							break;
						}
						
						return null;
					}
				}
			}
		}
		
		return htmltext;
	}
}
