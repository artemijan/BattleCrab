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
package quests.Q10273_GoodDayToFly;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Good Day to Fly (10273)
 * @author nonom
 */
public class Q10273_GoodDayToFly extends Quest
{
	// NPC
	private static final int LEKON = 32557;
	
	// Monsters
	private static final int[] MOBS =
	{
		22614, // Vulture Rider
		22615, // Vulture Rider
	};
	
	// Item
	private static final int MARK = 13856;
	
	// Skills
	private static final SkillHolder AURA_BIRD_FALCON = new SkillHolder(5982, 1);
	private static final SkillHolder AURA_BIRD_OWL = new SkillHolder(5983, 1);
	
	public Q10273_GoodDayToFly()
	{
		super(10273);
		addStartNpc(LEKON);
		addTalkId(LEKON);
		addKillId(MOBS);
		registerQuestItems(MARK);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "32557-06.htm":
			{
				qs.startQuest();
				break;
			}
			case "32557-09.html":
			{
				qs.set("transform", "1");
				AURA_BIRD_FALCON.getSkill().applyEffects(player, player);
				break;
			}
			case "32557-10.html":
			{
				qs.set("transform", "2");
				AURA_BIRD_OWL.getSkill().applyEffects(player, player);
				break;
			}
			case "32557-13.html":
			{
				switch (qs.getInt("transform"))
				{
					case 1:
					{
						AURA_BIRD_FALCON.getSkill().applyEffects(player, player);
						break;
					}
					case 2:
					{
						AURA_BIRD_OWL.getSkill().applyEffects(player, player);
						break;
					}
				}
				break;
			}
		}
		
		return event;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs == null) || !qs.isStarted())
		{
			return;
		}
		
		final long count = getQuestItemsCount(killer, MARK);
		if (qs.isCond(1) && (count < 5))
		{
			giveItems(killer, MARK, 1);
			if (count == 4)
			{
				qs.setCond(2, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		final int transform = qs.getInt("transform");
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				htmltext = "32557-0a.html";
				break;
			}
			case State.CREATED:
			{
				htmltext = (player.getLevel() < 75) ? "32557-00.html" : "32557-01.htm";
				break;
			}
			default:
			{
				if (getQuestItemsCount(player, MARK) >= 5)
				{
					htmltext = "32557-14.html";
					if (transform == 1)
					{
						giveItems(player, 13553, 1);
					}
					else if (transform == 2)
					{
						giveItems(player, 13554, 1);
					}
					
					giveItems(player, 13857, 1);
					addExpAndSp(player, 25160, 2525);
					qs.exitQuest(false, true);
				}
				else if (transform == 0)
				{
					htmltext = "32557-07.html";
				}
				else
				{
					htmltext = "32557-11.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
}
