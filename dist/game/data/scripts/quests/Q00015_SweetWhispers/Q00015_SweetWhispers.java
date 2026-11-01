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
package quests.Q00015_SweetWhispers;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Sweet Whispers (15)
 * @author nonom
 */
public class Q00015_SweetWhispers extends Quest
{
	// NPCs
	private static final int VLADIMIR = 31302;
	private static final int HIERARCH = 31517;
	private static final int M_NECROMANCER = 31518;
	
	// Misc
	private static final int MIN_LEVEL = 60;
	
	public Q00015_SweetWhispers()
	{
		super(15);
		addStartNpc(VLADIMIR);
		addTalkId(VLADIMIR, HIERARCH, M_NECROMANCER);
		addCondMinLevel(MIN_LEVEL, "31302-00a.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "31302-01.html":
			{
				qs.startQuest();
				break;
			}
			case "31518-01.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2);
				}
				break;
			}
			case "31517-01.html":
			{
				if (qs.isCond(2))
				{
					addExpAndSp(player, 714215, 171);
					qs.exitQuest(false, true);
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
				if (npcId == VLADIMIR)
				{
					htmltext = "31302-00.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npcId)
				{
					case VLADIMIR:
					{
						if (qs.isCond(1))
						{
							htmltext = "31302-01a.html";
						}
						break;
					}
					case M_NECROMANCER:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "31518-00.html";
								break;
							}
							case 2:
							{
								htmltext = "31518-01a.html";
								break;
							}
						}
						break;
					}
					case HIERARCH:
					{
						if (qs.isCond(2))
						{
							htmltext = "31517-00.html";
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
