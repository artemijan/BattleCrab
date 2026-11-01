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
package quests.Q10882_VictoryCollection;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10879_ExaltedGuideToPower.Q10879_ExaltedGuideToPower;

/**
 * Victory Collection (10882)
 * @URL https://l2wiki.com/Victory_Collection
 * @author CostyKiller
 */
public class Q10882_VictoryCollection extends Quest
{
	// Npcs
	private static final int SETTLEN = 34180;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		// TODO: Add Kelbim and Kain Instance Monsters
		// TODO: Add Story books drop to RBs
		// Tauti Instance Monsters
		23680, // Flame Golem
		23709, // Flame Scarab
		23683, // Seal Archangel
		23685, // Seal Angel
		
		// Freya Instance Monsters
		23686, // Frost Golem
		23687, // Glacier Frostbringer
		23703, // Ice Knight
		23689 // Freya
	
	};
	
	// Items
	private static final int SETTLEN_CERTIFICATE = 47837;
	private static final ItemHolder LIONEL_HUNTERS_LIST_PART_5 = new ItemHolder(47834, 1);
	private static final int STORY_BOOK_OF_TAUTI = 47847;
	private static final int STORY_BOOK_OF_KELBIM = 47848;
	private static final int STORY_BOOK_OF_FREYA = 47849;
	private static final int STORY_BOOK_OF_KAIN_VAN_HALTER = 47850;
	private static final int COVERED_PAGE = 48930;
	
	// Misc
	private static final int MIN_LEVEL = 104;
	
	public Q10882_VictoryCollection()
	{
		super(10882);
		addStartNpc(SETTLEN);
		addTalkId(SETTLEN);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "34180-00.htm");
		addCondStartedQuest(Q10879_ExaltedGuideToPower.class.getSimpleName(), "34180-00.htm");
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
			case "34180-02.htm":
			case "34180-04.html":
			{
				htmltext = event;
				break;
			}
			case "34180-03.html":
			{
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_5))
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "34180-06.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						giveItems(player, SETTLEN_CERTIFICATE, 1);
						addExpAndSp(player, 69661122560L, 709661122);
						qs.exitQuest(false, true);
						
						final Quest mainQ = ScriptManager.getInstance().getScript(Q10879_ExaltedGuideToPower.class.getSimpleName());
						if (mainQ != null)
						{
							mainQ.notifyEvent("SUBQUEST_FINISHED_NOTIFY", npc, player);
						}
						
						htmltext = event;
					}
					else
					{
						htmltext = getNoQuestLevelRewardMsg(player);
					}
					break;
				}
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
				if ((npc.getId() == SETTLEN) && (hasItem(player, LIONEL_HUNTERS_LIST_PART_5)))
				{
					htmltext = "34180-01.htm";
				}
				else
				{
					htmltext = "noItem.html";
				}
				break;
			}
			case State.STARTED:
			{
				if ((npc.getId() == SETTLEN) && qs.isCond(2) && (hasQuestItems(player, STORY_BOOK_OF_TAUTI, STORY_BOOK_OF_KELBIM, STORY_BOOK_OF_FREYA, STORY_BOOK_OF_KAIN_VAN_HALTER)))
				{
					htmltext = "34180-05.htm";
				}
				else
				{
					htmltext = "34180-04.html";
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
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE) && ArrayUtil.contains(MONSTERS, npc.getId()))
		{
			giveItems(player, COVERED_PAGE, 1);
			playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			
			if (getQuestItemsCount(player, COVERED_PAGE) >= 24)
			{
				qs.setCond(2, true);
			}
		}
	}
}
