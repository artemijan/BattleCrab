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
package quests.Q10883_ImmortalHonor;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10879_ExaltedGuideToPower.Q10879_ExaltedGuideToPower;

/**
 * Immortal Honor (10883)
 * @URL https://l2wiki.com/Immortal_Honor
 * @author CostyKiller
 */
public class Q10883_ImmortalHonor extends Quest
{
	// Npcs
	private static final int KEUCEREUS_ADMIRAL = 32548;
	
	// Raids
	private static final int CAMILLE = 26236;
	private static final int KAIN_VAN_HALTER = 26235;
	
	// Items
	private static final int KEUCEREUS_CERTIFICATE = 47837;
	private static final ItemHolder LIONEL_HUNTERS_LIST_PART_5 = new ItemHolder(47834, 1);
	
	// Misc
	private static final int MIN_LEVEL = 104;
	
	public Q10883_ImmortalHonor()
	{
		super(10883);
		addStartNpc(KEUCEREUS_ADMIRAL);
		addTalkId(KEUCEREUS_ADMIRAL);
		addKillId(CAMILLE, KAIN_VAN_HALTER);
		addCondMinLevel(MIN_LEVEL, "32548-00.htm");
		addCondStartedQuest(Q10879_ExaltedGuideToPower.class.getSimpleName(), "32548-00.htm");
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
			case "32548-02.htm":
			case "32548-03.htm":
			{
				htmltext = event;
				break;
			}
			case "32548-04.html":
			{
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_5))
				{
					qs.startQuest();
					qs.set(Integer.toString(CAMILLE), "false");
					qs.set(Integer.toString(KAIN_VAN_HALTER), "false");
					htmltext = event;
				}
				break;
			}
			case "32548-07.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						giveItems(player, KEUCEREUS_CERTIFICATE, 1);
						addExpAndSp(player, 104491683840L, 1064491683);
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
				if ((npc.getId() == KEUCEREUS_ADMIRAL) && (hasItem(player, LIONEL_HUNTERS_LIST_PART_5)))
				{
					htmltext = "32548-01.htm";
				}
				else
				{
					htmltext = "noItem.html";
				}
				break;
			}
			case State.STARTED:
			{
				if ((npc.getId() == KEUCEREUS_ADMIRAL) && qs.get(Integer.toString(CAMILLE)).equals("true") && qs.get(Integer.toString(KAIN_VAN_HALTER)).equals("true"))
				{
					htmltext = "32548-06.htm";
				}
				else
				{
					htmltext = "32548-05.html";
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
		if ((qs != null) && qs.isStarted() && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			qs.set(Integer.toString(npc.getId()), "true");
			playSound(player, QuestSound.ITEMSOUND_QUEST_MIDDLE);
			
			// notifyKill(npc, player, isSummon);
			sendNpcLogList(player);
		}
	}
}
