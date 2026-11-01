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
package quests.Q10456_OperationRescue;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

import quests.Q10455_ElikiasLetter.Q10455_ElikiasLetter;

/**
 * Operation: Rescue (10456)
 * @URL https://l2wiki.com/Operation:_Rescue
 * @author Gigi
 */
public class Q10456_OperationRescue extends Quest
{
	// NPCs
	private static final int DEVIANNE = 31590;
	private static final int[] MONSTERS =
	{
		23354, // Decay Hannibal
		23355, // Armor Beast
		23356, // Klein Soldier
		23357, // Disorder Warrior
		23358, // Blow Archer
		23360, // Bizuard
		23361, // Mutated Fly
		23362, // Amos Soldier
		23363, // Amos Officer
		23364, // Amos Master
		23365 // Ailith Hunter
	};
	
	// Misc
	private static final int MIN_LEVEL = 99;
	
	public Q10456_OperationRescue()
	{
		super(10456);
		addStartNpc(DEVIANNE);
		addTalkId(DEVIANNE);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "31590-00.htm");
		addFactionLevel(Faction.BLACKBIRD_CLAN, 2, "31590-05.html");
		addCondCompletedQuest(Q10455_ElikiasLetter.class.getSimpleName(), "31590-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		switch (event)
		{
			case "31590-02.htm":
			case "31590-03.htm":
			{
				htmltext = event;
				break;
			}
			case "31590-04.htm":
			{
				qs.startQuest();
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_HOPE_THEY_ARE_SAFE);
				htmltext = event;
				break;
			}
			case "31590-08.html":
			{
				if (qs.isCond(2) && (player.getLevel() >= MIN_LEVEL))
				{
					addExpAndSp(player, 1_507_456_500, 3_617_880);
					giveAdena(player, 659_250, false);
					qs.exitQuest(QuestType.ONE_TIME, true);
					htmltext = event;
				}
				else
				{
					htmltext = getNoQuestLevelRewardMsg(player);
				}
				break;
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
				htmltext = "31590-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "31590-06.html" : "31590-07.html";
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
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && (npc.getTitleString() == NpcStringId.ABNORMAL_MAGIC_CIRCLE) && (getRandom(100) < 5))
		{
			qs.setCond(2, true);
		}
	}
}
