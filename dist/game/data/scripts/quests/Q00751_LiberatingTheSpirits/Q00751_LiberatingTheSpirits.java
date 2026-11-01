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
package quests.Q00751_LiberatingTheSpirits;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Liberating the Spirits (00751)
 * @URL https://l2wiki.com/Liberating_the_Spirits
 * @author Gigi
 */
public class Q00751_LiberatingTheSpirits extends Quest
{
	// Npc
	private static final int RODERIK = 30631;
	
	// Monster's
	private static final int SCALDISECT = 23212;
	private static final int[] MOBS =
	{
		23199,
		23201,
		23202,
		23200,
		23203,
		23204,
		23205,
		23206,
		23207,
		23208,
		23209,
		23242,
		23243,
		23244,
		23245
	};
	
	// Item's
	private static final int DEADMANS_FLESH = 34971;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q00751_LiberatingTheSpirits()
	{
		super(751);
		addStartNpc(RODERIK);
		addTalkId(RODERIK);
		addKillId(SCALDISECT);
		addKillId(MOBS);
		registerQuestItems(DEADMANS_FLESH);
		addCondMinLevel(MIN_LEVEL, "lvl.htm");
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
			case "30631-1.htm":
			case "30631-2.htm":
			{
				htmltext = event;
				break;
			}
			case "30631-3.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30631-5.html":
			{
				takeItems(player, DEADMANS_FLESH, -1);
				addExpAndSp(player, 600000000, 144000);
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
		if (npc.getId() == RODERIK)
		{
			switch (qs.getState())
			{
				case State.COMPLETED:
				{
					if (!qs.isNowAvailable())
					{
						htmltext = "30631-0.htm";
						break;
					}
					
					qs.setState(State.CREATED);
					// fallthrough
				}
				case State.CREATED:
				{
					htmltext = "30631.htm";
					break;
				}
				case State.STARTED:
				{
					if (qs.isCond(1))
					{
						htmltext = "30631-3a.html";
					}
					else if (qs.isCond(2))
					{
						htmltext = "30631-4.html";
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
			if ((giveItemRandomly(killer, DEADMANS_FLESH, 1, 40, 0.2, true)) && (qs.getMemoState() < 1))
			{
				qs.setMemoState(1);
				showOnScreenMsg(killer, NpcStringId.SUMMON_SCALDISECT_OF_HELLFIRE, ExShowScreenMessage.TOP_CENTER, 6000);
				addSpawn(SCALDISECT, npc.getX() + 100, npc.getY() + 100, npc.getZ(), 0, false, 120000);
			}
			else if ((qs.isMemoState(1)) && (getQuestItemsCount(killer, DEADMANS_FLESH) >= 40) && (npc.getId() == SCALDISECT))
			{
				int kills = qs.getInt(Integer.toString(SCALDISECT));
				if (kills < 1)
				{
					kills++;
					qs.set(Integer.toString(SCALDISECT), kills);
				}
				
				final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
				log.addNpc(SCALDISECT, qs.getInt(Integer.toString(SCALDISECT)));
				qs.getPlayer().sendPacket(log);
				
				if ((qs.getInt(Integer.toString(SCALDISECT)) >= 1))
				{
					qs.setCond(2, true);
				}
			}
		}
	}
}
