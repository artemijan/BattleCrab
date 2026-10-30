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
package quests.Q10378_WeedingWork;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;
import org.l2jmobius.gameserver.util.LocationUtil;

/**
 * Weeding Work (10378)
 * @URL https://l2wiki.com/Weeding_Work
 * @author Gigi
 */
public class Q10378_WeedingWork extends Quest
{
	// NPCs
	private static final int DADFPHYNA = 33697;
	
	// Monster's
	private static final int MANDRAGORA_OF_JOY_AND_SORROW = 23210;
	private static final int MANDRAGORA_OF_PRAYER = 23211;
	
	// Items
	private static final int MANDRAGORA_ROOT = 34975;
	private static final int MANDRAGORA_STEM = 34974;
	private static final int SOE_GUILLOTINE_FORTRESS = 35292;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10378_WeedingWork()
	{
		super(10378);
		addStartNpc(DADFPHYNA);
		addTalkId(DADFPHYNA);
		addKillId(MANDRAGORA_OF_JOY_AND_SORROW, MANDRAGORA_OF_PRAYER);
		registerQuestItems(MANDRAGORA_ROOT, MANDRAGORA_STEM);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
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
			case "33697-01a.htm":
			case "33697-01b.htm":
			{
				htmltext = event;
				break;
			}
			case "33697-02.htm":
			{
				qs.startQuest();
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, DADFPHYNA, NpcStringId.PLEASE_HELP_US_DISCOVER_THE_CAUSE_OF_THIS_CHAOS));
				htmltext = event;
				break;
			}
			case "33697-05.html":
			{
				giveAdena(player, 3000000, true);
				giveItems(player, SOE_GUILLOTINE_FORTRESS, 2);
				addExpAndSp(player, 845059770, 202814);
				qs.exitQuest(false, true);
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, DADFPHYNA, NpcStringId.THANK_YOU_IT_WILL_CERTAINLY_HELP_THE_RESEARCH));
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
		if (qs.isCreated())
		{
			htmltext = "33697-01.htm";
		}
		else if (qs.isStarted())
		{
			if (qs.isCond(1))
			{
				htmltext = "33697-03.html";
			}
			else if (qs.isCond(2))
			{
				htmltext = "33697-04.html";
			}
		}
		else if (qs.isCompleted())
		{
			htmltext = getNoQuestMsg(player);
		}
		
		return htmltext;
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false))
		{
			switch (npc.getId())
			{
				case MANDRAGORA_OF_PRAYER:
				{
					if (getQuestItemsCount(player, MANDRAGORA_ROOT) < 5)
					{
						giveItems(player, MANDRAGORA_ROOT, 1);
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case MANDRAGORA_OF_JOY_AND_SORROW:
				{
					if (getQuestItemsCount(player, MANDRAGORA_STEM) < 5)
					{
						giveItems(player, MANDRAGORA_STEM, 1);
						playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
			
			if ((getQuestItemsCount(player, MANDRAGORA_ROOT) >= 5) && (getQuestItemsCount(player, MANDRAGORA_STEM) >= 5))
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
}
