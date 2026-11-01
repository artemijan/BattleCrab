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
package quests.Q10274_CollectingInTheAir;

import java.util.Collection;

import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.skill.Skill;

import quests.Q10273_GoodDayToFly.Q10273_GoodDayToFly;

/**
 * Collecting in the Air (10274)
 * @author nonom
 */
public class Q10274_CollectingInTheAir extends Quest
{
	// NPC
	private static final int LEKON = 32557;
	
	// Items
	private static final int SCROLL = 13844;
	private static final int RED = 13858;
	private static final int BLUE = 13859;
	private static final int GREEN = 13860;
	
	// Monsters
	private static final int[] MOBS =
	{
		18684, // Red Star Stone
		18685, // Red Star Stone
		18686, // Red Star Stone
		18687, // Blue Star Stone
		18688, // Blue Star Stone
		18689, // Blue Star Stone
		18690, // Green Star Stone
		18691, // Green Star Stone
		18692, // Green Star Stone
	};
	
	public Q10274_CollectingInTheAir()
	{
		super(10274);
		addStartNpc(LEKON);
		addTalkId(LEKON);
		addSkillSeeId(MOBS);
		registerQuestItems(SCROLL, RED, BLUE, GREEN);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		if (event.equals("32557-03.html"))
		{
			qs.startQuest();
			giveItems(player, SCROLL, 8);
		}
		
		return event;
	}
	
	@Override
	public void onSkillSee(Npc npc, Player caster, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		final QuestState qs = getQuestState(caster, false);
		if ((qs == null) || !qs.isStarted())
		{
			return;
		}
		
		if (qs.isCond(1) && (skill.getId() == 2630))
		{
			switch (npc.getId())
			{
				case 18684:
				case 18685:
				case 18686:
				{
					giveItems(caster, RED, 1);
					break;
				}
				case 18687:
				case 18688:
				case 18689:
				{
					giveItems(caster, BLUE, 1);
					break;
				}
				case 18690:
				case 18691:
				case 18692:
				{
					giveItems(caster, GREEN, 1);
					break;
				}
			}
			
			playSound(caster, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			npc.doDie(caster);
		}
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				htmltext = "32557-0a.html";
				break;
			}
			case State.CREATED:
			{
				qs = player.getQuestState(Q10273_GoodDayToFly.class.getSimpleName());
				if (qs != null)
				{
					htmltext = ((player.getLevel() >= 75) && qs.isCompleted()) ? "32557-01.htm" : "32557-00.html";
				}
				else
				{
					htmltext = "32557-00.html";
				}
				break;
			}
			case State.STARTED:
			{
				if ((getQuestItemsCount(player, RED) + getQuestItemsCount(player, BLUE) + getQuestItemsCount(player, GREEN)) >= 8)
				{
					htmltext = "32557-05.html";
					giveItems(player, 13728, 1);
					addExpAndSp(player, 25160, 2525);
					qs.exitQuest(false, true);
				}
				else
				{
					htmltext = "32557-04.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
