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
package quests.Q10388_ConspiracyBehindDoor;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * @author hlwrave
 */
public class Q10388_ConspiracyBehindDoor extends Quest
{
	// NPCs
	private static final int ELIA = 31329;
	private static final int KARGOS = 33821;
	private static final int HICHEN = 33820;
	private static final int RAZDEN = 33803;
	
	// Item
	private static final int VISITORS_BADGE = 8064;
	
	// Misc
	private static final int MIN_LEVEL = 97;
	
	public Q10388_ConspiracyBehindDoor()
	{
		super(10388);
		addStartNpc(ELIA);
		addTalkId(ELIA, KARGOS, HICHEN, RAZDEN);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "go.html":
			{
				qs.startQuest();
				break;
			}
			case "toCond2.html":
			{
				qs.setCond(2, true);
				break;
			}
			case "toCond3.html":
			{
				qs.setCond(0);
				qs.setCond(3, true);
				giveItems(player, VISITORS_BADGE, 1);
				break;
			}
			case "final.html":
			{
				addExpAndSp(player, 29638350, 2963835);
				qs.exitQuest(false, true);
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
		
		final int npcId = npc.getId();
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
			case State.CREATED:
			{
				if (npcId == ELIA)
				{
					htmltext = player.getLevel() >= MIN_LEVEL ? "start.htm" : "nolvl.html";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npcId)
				{
					case KARGOS:
					{
						if (qs.isCond(1))
						{
							htmltext = "cond1.html";
						}
						break;
					}
					case HICHEN:
					{
						if (qs.isCond(2))
						{
							htmltext = "cond2.html";
						}
						break;
					}
					case RAZDEN:
					{
						if (qs.isCond(3))
						{
							htmltext = "cond3.html";
						}
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
}
