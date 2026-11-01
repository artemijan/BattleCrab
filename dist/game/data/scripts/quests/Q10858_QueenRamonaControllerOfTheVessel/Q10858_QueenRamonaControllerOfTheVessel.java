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
package quests.Q10858_QueenRamonaControllerOfTheVessel;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.LocationUtil;

import quests.Q10856_SuperionAppears.Q10856_SuperionAppears;

/**
 * Queen Ramona, Controller of the Vessel (10858)
 * @author Kazumi
 */
public class Q10858_QueenRamonaControllerOfTheVessel extends Quest
{
	// NPCs
	private static final int KEKROPUS = 34222;
	private static final int RAMONA = 26143;
	
	// Item
	private static final int SUPER_GIANT_CHAPTER_1 = 46150;
	
	// Misc
	private static final int MIN_LEVEL = 102;
	
	public Q10858_QueenRamonaControllerOfTheVessel()
	{
		super(10858);
		addStartNpc(KEKROPUS);
		addTalkId(KEKROPUS);
		addKillId(RAMONA);
		addCondMinLevel(MIN_LEVEL, "leader_kekrops_q10858_02.htm");
		addCondCompletedQuest(Q10856_SuperionAppears.class.getSimpleName(), "leader_kekrops_q10858_02.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		String htmltext = null;
		switch (event)
		{
			case "leader_kekrops_q10858_03.htm":
			case "leader_kekrops_q10858_04.htm":
			{
				htmltext = event;
				break;
			}
			case "leader_kekrops_q10858_05.htm":
			{
				qs.startQuest();
				break;
			}
			case "leader_kekrops_q10858_08.htm":
			{
				if (qs.isCond(2))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						qs.exitQuest(false, true);
						giveItems(player, SUPER_GIANT_CHAPTER_1, 1);
						addExpAndSp(player, 1630746824, 14221620);
						break;
					}
					
					htmltext = getNoQuestLevelRewardMsg(player);
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "leader_kekrops_q10858_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "leader_kekrops_q10858_06.htm";
						break;
					}
					case 2:
					{
						htmltext = "leader_kekrops_q10858_07.htm";
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
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if ((qs != null) && qs.isCond(1) && LocationUtil.checkIfInRange(1500, npc, qs.getPlayer(), false))
		{
			qs.setCond(2, true);
		}
	}
}
