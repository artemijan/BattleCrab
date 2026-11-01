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
package quests.Q10310_TwistedCreationTree;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

import quests.Q10302_UnsettlingShadowAndRumors.Q10302_UnsettlingShadowAndRumors;

/**
 * Twisted Creation Tree (10310)
 * @URL https://l2wiki.com/Creation_of_Twisted_Spiral
 * @author Gigi
 */
public class Q10310_TwistedCreationTree extends Quest
{
	// NPCs
	private static final int SELINA = 33032;
	private static final int GORFINA = 33031;
	
	// Monsters
	private static final int GARDEN_SENTRY = 22947;
	private static final int GARDEN_SCOUT = 22948;
	private static final int GARDEN_COMMANDER = 22949;
	private static final int OUTDOOR_GARDENER = 22950;
	private static final int GARDEN_DESTROYER = 22951;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q10310_TwistedCreationTree()
	{
		super(10310);
		addStartNpc(SELINA);
		addTalkId(SELINA, GORFINA);
		addKillId(GARDEN_SENTRY, GARDEN_SCOUT, GARDEN_COMMANDER, OUTDOOR_GARDENER, GARDEN_DESTROYER);
		addCondMinLevel(MIN_LEVEL, "33032-00.htm");
		addCondCompletedQuest(Q10302_UnsettlingShadowAndRumors.class.getSimpleName(), "33032-00.htm");
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
			case "33032-02.htm":
			case "33032-03.htm":
			case "33032-07.html":
			case "33031-02.html":
			case "33031-06.html":
			{
				htmltext = event;
				break;
			}
			case "33032-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33031-03.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			default:
			{
				if (qs.isCond(3) && event.startsWith("giveReward_"))
				{
					final int itemId = Integer.parseInt(event.replace("giveReward_", ""));
					qs.exitQuest(false, true);
					giveAdena(player, 3424540, false);
					giveItems(player, itemId, 11);
					addExpAndSp(player, 50178765, 12042);
					htmltext = "33031-07.html";
				}
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
				if (npc.getId() == SELINA)
				{
					htmltext = "33032-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case SELINA:
					{
						if (qs.isCond(1))
						{
							htmltext = "33032-05.html";
						}
						else if (qs.getCond() > 1)
						{
							htmltext = "33032-06.html";
						}
						break;
					}
					case GORFINA:
					{
						switch (qs.getCond())
						{
							case 1:
							{
								htmltext = "33031-01.html";
								break;
							}
							case 2:
							{
								htmltext = "33031-04.html";
								break;
							}
							case 3:
							{
								htmltext = "33031-05.html";
								break;
							}
						}
						break;
					}
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "complete.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 2, 3, npc);
		if (qs != null)
		{
			switch (npc.getId())
			{
				case GARDEN_SENTRY:
				{
					int kills = qs.getInt(Integer.toString(GARDEN_SENTRY));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(GARDEN_SENTRY), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case GARDEN_SCOUT:
				{
					int kills = qs.getInt(Integer.toString(GARDEN_SCOUT));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(GARDEN_SCOUT), kills);
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
				case OUTDOOR_GARDENER:
				{
					int kills = qs.getInt(Integer.toString(OUTDOOR_GARDENER));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(OUTDOOR_GARDENER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case GARDEN_DESTROYER:
				{
					int kills = qs.getInt(Integer.toString(GARDEN_DESTROYER));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(GARDEN_DESTROYER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(GARDEN_SENTRY, qs.getInt(Integer.toString(GARDEN_SENTRY)));
			log.addNpc(GARDEN_SCOUT, qs.getInt(Integer.toString(GARDEN_SCOUT)));
			log.addNpc(GARDEN_COMMANDER, qs.getInt(Integer.toString(GARDEN_COMMANDER)));
			log.addNpc(OUTDOOR_GARDENER, qs.getInt(Integer.toString(OUTDOOR_GARDENER)));
			log.addNpc(GARDEN_DESTROYER, qs.getInt(Integer.toString(GARDEN_DESTROYER)));
			qs.getPlayer().sendPacket(log);
			if ((qs.getInt(Integer.toString(GARDEN_SENTRY)) >= 10) && (qs.getInt(Integer.toString(GARDEN_SCOUT)) >= 10) && (qs.getInt(Integer.toString(GARDEN_COMMANDER)) >= 10) && (qs.getInt(Integer.toString(GARDEN_DESTROYER)) >= 10) && (qs.getInt(Integer.toString(GARDEN_DESTROYER)) >= 10))
			{
				qs.setCond(3, true);
			}
		}
	}
}
