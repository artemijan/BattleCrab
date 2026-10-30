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
package quests.Q10566_BestChoice;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Best Choice (10566)
 * @URL https://l2wiki.com/Best_Choice
 * @author Werum, NightBR
 */
public class Q10566_BestChoice extends Quest
{
	// NPC
	private static final int HERPHAH = 34362;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	// Items
	private static final int CERTIFICATE_SANTIAGO = 48173;
	private static final int CERTIFICATE_RUPIO = 48174;
	private static final int CERTIFICATE_FLUTTER = 48175;
	private static final int CERTIFICATE_VINCENZ = 48176;
	private static final int CERTIFICATE_FERRIS = 48177;
	private static final int HERPHAHS_MISSION_LIST = 48172;
	
	// Rewards
	private static final int HERPHAHS_SUPPORT_BOX = 48250;
	
	public Q10566_BestChoice()
	{
		super(10566);
		addStartNpc(HERPHAH);
		addTalkId(HERPHAH);
		addCondMinLevel(MIN_LEVEL, "noLevel.html");
		registerQuestItems(CERTIFICATE_SANTIAGO, CERTIFICATE_RUPIO, CERTIFICATE_FLUTTER, CERTIFICATE_VINCENZ, CERTIFICATE_FERRIS, HERPHAHS_MISSION_LIST);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		String htmltext = null;
		switch (event)
		{
			case "34362-02.htm":
			case "34362-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34362-04.htm":
			{
				qs.startQuest();
				giveItems(player, HERPHAHS_MISSION_LIST, 1);
				break;
			}
			case "34362-07.html":
			{
				giveItems(player, HERPHAHS_SUPPORT_BOX, 1);
				qs.exitQuest(QuestType.ONE_TIME, true);
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
				htmltext = (player.hasPremiumStatus()) ? "34362-01.htm" : "34362-99.html";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					// Check if player has the necessary quest items to complete the quest
					if (hasQuestItems(player, CERTIFICATE_SANTIAGO, CERTIFICATE_RUPIO, CERTIFICATE_FLUTTER, CERTIFICATE_VINCENZ, CERTIFICATE_FERRIS))
					{
						qs.setCond(2, true);
						htmltext = "34362-06.html";
					}
					else
					{
						htmltext = "34362-05.html";
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
