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
package quests.Q00469_SuspiciousGardener;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

/**
 * Suspicious Gardener (469)
 * @URL https://l2wiki.com/Suspicious_Gardener
 * @author Gigi
 */
public class Q00469_SuspiciousGardener extends Quest
{
	// NPC
	private static final int GOFINA = 33031;
	
	// Monsters
	private static final int APHERIUS_LOOKOUT_BEWILDERED = 22964;
	
	// Items
	private static final ItemHolder CERTIFICATE_OF_LIFE = new ItemHolder(30385, 2); // Certificate of Life
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q00469_SuspiciousGardener()
	{
		super(469);
		addStartNpc(GOFINA);
		addTalkId(GOFINA);
		addKillId(APHERIUS_LOOKOUT_BEWILDERED);
		addCondMinLevel(MIN_LEVEL, "no_level.html");
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
			case "33031-02.htm":
			{
				htmltext = event;
				break;
			}
			case "33031-03.htm":
			{
				qs.startQuest();
				qs.set(Integer.toString(APHERIUS_LOOKOUT_BEWILDERED), 0);
				htmltext = event;
				break;
			}
			case "33031-06.html":
			{
				giveItems(player, CERTIFICATE_OF_LIFE);
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
		if (npc.getId() == GOFINA)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "33031-04.html";
						break;
					}
					
					qs.setState(State.CREATED);
					// fallthrough
				}
				case State.CREATED:
				{
					htmltext = "33031-01.htm";
				}
					break;
				case State.STARTED:
				{
					switch (qs.getCond())
					{
						case 1:
						{
							htmltext = "33031-07.html";
							break;
						}
						case 2:
						{
							htmltext = "33031-05.html";
							break;
						}
					}
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
			int kills = qs.getInt(Integer.toString(APHERIUS_LOOKOUT_BEWILDERED));
			if (kills < 30)
			{
				kills++;
				qs.set(Integer.toString(APHERIUS_LOOKOUT_BEWILDERED), kills);
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(APHERIUS_LOOKOUT_BEWILDERED, qs.getInt(Integer.toString(APHERIUS_LOOKOUT_BEWILDERED)));
			qs.getPlayer().sendPacket(log);
			
			if ((qs.getInt(Integer.toString(APHERIUS_LOOKOUT_BEWILDERED)) >= 30))
			{
				qs.setCond(2, true);
			}
		}
	}
}
