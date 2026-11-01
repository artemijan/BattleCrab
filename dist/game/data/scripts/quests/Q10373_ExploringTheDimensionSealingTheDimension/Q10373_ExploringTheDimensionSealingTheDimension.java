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
package quests.Q10373_ExploringTheDimensionSealingTheDimension;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * @author Sero
 */
public class Q10373_ExploringTheDimensionSealingTheDimension extends Quest
{
	private static final int BELOA = 34227;
	private static final int RUNE_STONE = 39738;
	private static final int COMMANDO_BELT = 47044;
	private static final int REMNANT_OF_THE_RIFT = 46787;
	private static final int ZODIAC_AGATHION = 45577;
	private static final int MIN_LEVEL = 95;
	
	public Q10373_ExploringTheDimensionSealingTheDimension()
	{
		super(10373);
		addStartNpc(BELOA);
		addTalkId(BELOA);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
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
		
		if (event.equals("34227-04.htm"))
		{
			qs.startQuest();
			htmltext = event;
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (npc.getId() == BELOA)
		{
			switch (qs.getState())
			{
				case State.CREATED:
				{
					htmltext = "34227-05.htm";
					break;
				}
				case State.STARTED:
				{
					if (getQuestItemsCount(player, REMNANT_OF_THE_RIFT) >= 30)
					{
						takeItems(player, REMNANT_OF_THE_RIFT, -1);
						giveItems(player, COMMANDO_BELT, 1);
						giveItems(player, ZODIAC_AGATHION, 1);
						giveItems(player, RUNE_STONE, 1);
						addExpAndSp(player, 12113489880L, 12113460);
						qs.exitQuest(QuestType.ONE_TIME);
						htmltext = "30756-09.html";
					}
					else
					{
						htmltext = "34227-05a.htm";
					}
					break;
				}
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "34227-00a.htm";
						break;
					}
				}
			}
		}
		
		return htmltext;
	}
}
