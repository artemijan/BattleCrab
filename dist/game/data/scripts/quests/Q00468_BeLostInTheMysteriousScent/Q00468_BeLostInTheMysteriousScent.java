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
package quests.Q00468_BeLostInTheMysteriousScent;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

/**
 * Be Lost in the Mysterious Scent (468)
 * @URL https://l2wiki.com/Be_Lost_in_the_Mysterious_Scent
 * @author Gigi
 */
public class Q00468_BeLostInTheMysteriousScent extends Quest
{
	// NPCs
	private static final int SELINA = 33032;
	private static final int MOON_GARDEN_MANAGER = 22958;
	private static final int GARDEN_PROTECTOR = 22959;
	private static final int GARDEN_COMMANDER = 22962;
	private static final int MOON_GARDENER = 22960;
	
	// Item
	private static final int CERTIFICATE_OF_LIFE = 30385;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q00468_BeLostInTheMysteriousScent()
	{
		super(468);
		addStartNpc(SELINA);
		addTalkId(SELINA);
		addKillId(MOON_GARDEN_MANAGER, GARDEN_PROTECTOR, GARDEN_COMMANDER, MOON_GARDENER);
		addCondMinLevel(MIN_LEVEL, "32892-00a.html");
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
		
		switch (event)
		{
			case "33032-02.htm":
			{
				htmltext = event;
				break;
			}
			case "33032-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33032-06.html":
			{
				if (qs.isCond(2))
				{
					giveItems(player, CERTIFICATE_OF_LIFE, 2);
					qs.exitQuest(QuestType.DAILY, true);
				}
				
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
		if (npc.getId() == SELINA)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "32892-00.html";
						break;
					}
					
					qs.setState(State.CREATED);
					// fallthrough
				}
				case State.CREATED:
				{
					htmltext = "33032-01.htm";
					qs.isStarted();
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "33032-04.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "33032-05.html";
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if (qs != null)
		{
			switch (npc.getId())
			{
				case MOON_GARDEN_MANAGER:
				{
					int kills = qs.getInt(Integer.toString(MOON_GARDEN_MANAGER));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(MOON_GARDEN_MANAGER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case GARDEN_PROTECTOR:
				{
					int kills = qs.getInt(Integer.toString(GARDEN_PROTECTOR));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(GARDEN_PROTECTOR), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case GARDEN_COMMANDER:
				{
					int kills = qs.getInt(Integer.toString(GARDEN_COMMANDER));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(GARDEN_COMMANDER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case MOON_GARDENER:
				{
					int kills = qs.getInt(Integer.toString(MOON_GARDENER));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(MOON_GARDENER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(MOON_GARDEN_MANAGER, qs.getInt(Integer.toString(MOON_GARDEN_MANAGER)));
			log.addNpc(GARDEN_PROTECTOR, qs.getInt(Integer.toString(GARDEN_PROTECTOR)));
			log.addNpc(GARDEN_COMMANDER, qs.getInt(Integer.toString(GARDEN_COMMANDER)));
			log.addNpc(MOON_GARDENER, qs.getInt(Integer.toString(MOON_GARDENER)));
			qs.getPlayer().sendPacket(log);
			
			if ((qs.getInt(Integer.toString(MOON_GARDEN_MANAGER)) >= 10) && (qs.getInt(Integer.toString(GARDEN_PROTECTOR)) >= 10) && (qs.getInt(Integer.toString(GARDEN_COMMANDER)) >= 10) && (qs.getInt(Integer.toString(MOON_GARDENER)) >= 10))
			{
				qs.setCond(2, true);
			}
		}
	}
}
