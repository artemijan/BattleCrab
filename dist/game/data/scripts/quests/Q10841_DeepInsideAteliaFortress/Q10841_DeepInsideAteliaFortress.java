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
package quests.Q10841_DeepInsideAteliaFortress;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Deep Inside Atelia Fortress (10841)
 * @URL https://l2wiki.com/Deep_Inside_Atelia_Fortress
 * @author Gigi
 */
public class Q10841_DeepInsideAteliaFortress extends Quest
{
	// NPCs
	private static final int ELIKIA = 34057;
	private static final int KAYSIA = 34051;
	
	// Boss
	private static final int KELBIM = 26124;
	
	// Items
	private static final int KELBIM_ARMOR_PIECE = 46144;
	private static final int SPIRIT_STONE_HAIR_ACCESSORY = 45937;
	private static final int SUPERIOR_GIANTS_CODEX = 46151;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	
	public Q10841_DeepInsideAteliaFortress()
	{
		super(10841);
		addStartNpc(ELIKIA);
		addTalkId(ELIKIA, KAYSIA);
		addKillId(KELBIM);
		registerQuestItems(KELBIM_ARMOR_PIECE);
		addCondMinLevel(MIN_LEVEL, "34057-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "34057-02.htm":
			case "34057-03.htm":
			case "34057-04.htm":
			{
				htmltext = event;
				break;
			}
			case "34057-05.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34051-02.html":
			{
				giveItems(player, SPIRIT_STONE_HAIR_ACCESSORY, 1);
				giveItems(player, SUPERIOR_GIANTS_CODEX, 1);
				addExpAndSp(player, 7262301690L, 17429400);
				qs.exitQuest(false, true);
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
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "34057-01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case ELIKIA:
					{
						if (qs.isCond(1))
						{
							htmltext = "34057-06.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "34057-07.html";
						}
						break;
					}
					case KAYSIA:
					{
						if (qs.isCond(1))
						{
							htmltext = "34051-00.html";
						}
						else if (qs.isCond(2) && hasQuestItems(player, KELBIM_ARMOR_PIECE))
						{
							htmltext = "34051-01.html";
						}
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
			giveItems(player, KELBIM_ARMOR_PIECE, 1);
			qs.setCond(2, true);
		}
	}
}
