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
package quests.Q10843_AnomalyInTheEnchantedValley;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Anomaly in the Enchanted Valley (10843)
 * @URL https://l2wiki.com/Anomaly_in_the_Enchanted_Valley
 * @author Gigi
 */
public class Q10843_AnomalyInTheEnchantedValley extends Quest
{
	// NPCs
	private static final int CRONOS = 30610;
	private static final int MIMYU = 30747;
	
	// Items
	private static final int SOE = 46257; // Scroll of Escape: Enchanted Valley
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10843_AnomalyInTheEnchantedValley()
	{
		super(10843);
		addStartNpc(CRONOS);
		addTalkId(CRONOS, MIMYU);
		addCondMinLevel(MIN_LEVEL, "30610-00.htm");
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
			case "30610-02.htm":
			case "30610-03.htm":
			case "30610-04.htm":
			case "30747-02.html":
			{
				htmltext = event;
				break;
			}
			case "30610-05.htm":
			{
				qs.startQuest();
				giveItems(player, SOE, 1);
				showOnScreenMsg(player, NpcStringId.TALK_TO_MIMYU, ExShowScreenMessage.TOP_CENTER, 8000);
				htmltext = event;
				break;
			}
			case "30747-03.html":
			{
				giveItems(player, SOE, 3);
				qs.exitQuest(false, true);
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
			case State.CREATED:
			{
				if (npc.getId() == CRONOS)
				{
					htmltext = "30610-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case CRONOS:
					{
						if (qs.isCond(1))
						{
							htmltext = "30610-06.html";
						}
						break;
					}
					case MIMYU:
					{
						if (qs.isCond(1))
						{
							htmltext = "30747-01.html";
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
