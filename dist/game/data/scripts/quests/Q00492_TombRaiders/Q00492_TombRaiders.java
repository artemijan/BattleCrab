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
package quests.Q00492_TombRaiders;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;

/**
 * Tomb Raiders (492)
 * @URL https://l2wiki.com/Tomb_Raiders
 * @author Gigi
 */
public class Q00492_TombRaiders extends Quest
{
	// NPCs
	private static final int ZENYA = 32140;
	
	// Items
	private static final int RELICS_OF_THE_EMPIRE = 34769;
	
	// Reward
	private static final int EXP_REWARD = 300500;
	private static final int SP_REWARD = 75;
	
	// Misc
	private static final int MIN_LEVEL = 80;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		23193, // Apparition Destroyer (83)
		23194, // Apparition Assassin (83)
		23195, // Apparition Sniper (83)
		23196 // Apparition Wizard (83)
	};
	
	public Q00492_TombRaiders()
	{
		super(492);
		addStartNpc(ZENYA);
		addTalkId(ZENYA);
		registerQuestItems(RELICS_OF_THE_EMPIRE);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "noLevel.html");
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
			case "32140-02.htm":
			{
				htmltext = event;
				break;
			}
			case "32140-03.htm":
			{
				qs.startQuest();
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
		String htmltext = qs.isCompleted() ? getAlreadyCompletedMsg(player) : getNoQuestMsg(player);
		if ((npc.getId() == ZENYA) && !player.isSubClassActive() && !player.isDualClassActive() && (player.getPlayerClass().level() == 4))
		{
			return "noClass.html";
		}
		
		switch (npc.getId())
		{
			case ZENYA:
			{
				if (qs.isCreated())
				{
					htmltext = "32140-01.htm";
				}
				else if (qs.isStarted())
				{
					switch (qs.getCond())
					{
						case 1:
						{
							htmltext = "32140-05.html";
							break;
						}
						case 2:
						{
							if (qs.isCond(2) && (getQuestItemsCount(player, RELICS_OF_THE_EMPIRE) >= 50))
							{
								takeItems(player, RELICS_OF_THE_EMPIRE, 50);
								addExpAndSp(player, EXP_REWARD * player.getLevel(), SP_REWARD * player.getLevel());
								playSound(player, QuestSound.ITEMSOUND_QUEST_FINISH);
								qs.exitQuest(QuestType.DAILY, true);
								htmltext = "32140-04.html";
							}
							break;
						}
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(killer, npc, RELICS_OF_THE_EMPIRE, 1, 50, 0.30, true))
		{
			qs.setCond(2);
		}
	}
}
