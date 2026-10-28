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
package quests.Q10844_BloodyBattleSeizingSupplies;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * Bloody Battle - Seizing Supplies (10844)
 * @URL https://l2wiki.com/Bloody_Battle_-_Seizing_Supplies
 * @author Dmitri
 */
public class Q10844_BloodyBattleSeizingSupplies extends Quest
{
	// NPC
	private static final int ELIKIA = 34057;
	private static final int GLENKINCHIE = 34063;
	private static final int EMBRYO_SUPPLY_BOX = 34137;
	
	// Monsters
	private static final int FORTRESS_GUARDIAN_CAPTAIN = 23506;
	private static final int FORTRESS_RAIDER = 23505;
	
	// Items
	private static final int EMBRYO_SUPPLIES = 46282;
	
	// Misc
	private static final int MIN_LEVEL = 101;
	
	public Q10844_BloodyBattleSeizingSupplies()
	{
		super(10844);
		addStartNpc(ELIKIA);
		addTalkId(ELIKIA, GLENKINCHIE);
		addFirstTalkId(EMBRYO_SUPPLY_BOX);
		registerQuestItems(EMBRYO_SUPPLIES);
		addCondMinLevel(MIN_LEVEL, "34057-00.htm");
		addFactionLevel(Faction.KINGDOM_ROYAL_GUARDS, 2, "34057-00.htm");
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
			case "34057-04.htm":
			case "34057-03.htm":
			case "34057-02.htm":
			case "34063-02.html":
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
			case "34063-03.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "open_box":
			{
				if (qs.isCond(2))
				{
					npc.deleteMe();
					if (getRandom(10) < 5)
					{
						if (qs.isCond(2) && (getQuestItemsCount(qs.getPlayer(), EMBRYO_SUPPLIES) < 19))
						{
							giveItems(player, EMBRYO_SUPPLIES, 1);
							playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						}
						else
						{
							giveItems(player, EMBRYO_SUPPLIES, 1);
							qs.setCond(3, true);
						}
						break;
					}
					
					final Npc captain = addSpawn(FORTRESS_GUARDIAN_CAPTAIN, npc, true, 120000, false);
					captain.setTitleString(NpcStringId.SUPPLY_GUARDS);
					addAttackPlayerDesire(captain, player);
					for (int i = 0; i < 2; i++)
					{
						final Npc raider = addSpawn(FORTRESS_RAIDER, npc, true, 120000, false);
						raider.setTitleString(NpcStringId.SUPPLY_GUARDS);
						addAttackPlayerDesire(raider, player);
					}
				}
				break;
			}
			case "34063-06.html":
			{
				if (qs.isCond(3))
				{
					addExpAndSp(player, 7262301690L, 17429400);
					qs.exitQuest(false, true);
					htmltext = event;
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
				if (npc.getId() == ELIKIA)
				{
					htmltext = "34057-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case ELIKIA:
					{
						if (qs.getCond() > 1)
						{
							htmltext = "34057-06.html";
						}
						break;
					}
					case GLENKINCHIE:
					{
						if (qs.isCond(1))
						{
							htmltext = "34063-01.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "34063-04.html";
						}
						else if (qs.isCond(3))
						{
							htmltext = "34063-05.html";
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
	public String onFirstTalk(Npc npc, Player player)
	{
		return "34137.html";
	}
}
