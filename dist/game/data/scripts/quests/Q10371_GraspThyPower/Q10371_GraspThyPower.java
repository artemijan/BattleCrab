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
package quests.Q10371_GraspThyPower;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;

import quests.Q10370_MenacingTimes.Q10370_MenacingTimes;

/**
 * Grasp Thy Power (10371)
 * @URL https://l2wiki.com/Grasp_Thy_Power
 * @author Gigi
 */
public class Q10371_GraspThyPower extends Quest
{
	// NPCs
	private static final int GERKENSHTEIN = 33648;
	
	// Monster's
	private static final int SUCCUBUS_SOLDIER = 23181;
	private static final int SUCCUBUS_WARRIOR = 23182;
	private static final int SUCCUBUS_ARCHER = 23183;
	private static final int SUCCUBUS_SHAMAN = 23184;
	private static final int BLOODY_SUCCUBUS = 23185;
	
	// Items
	private static final ItemHolder ADENA = new ItemHolder(57, 484990);
	
	// Reward
	private static final int EXP_REWARD = 22641900;
	private static final int SP_REWARD = 5434;
	
	// Misc
	private static final int MIN_LEVEL = 76;
	private static final int MAX_LEVEL = 81;
	
	public Q10371_GraspThyPower()
	{
		super(10371);
		addStartNpc(GERKENSHTEIN);
		addTalkId(GERKENSHTEIN);
		addKillId(SUCCUBUS_SOLDIER, SUCCUBUS_WARRIOR, SUCCUBUS_ARCHER, SUCCUBUS_SHAMAN, BLOODY_SUCCUBUS);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "no_level.html");
		addCondCompletedQuest(Q10370_MenacingTimes.class.getSimpleName(), "restriction.html");
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
			case "33648-02.htm":
			case "33648-03.htm":
			case "33648-07.html":
			{
				htmltext = event;
				break;
			}
			case "33648-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "33648-08.html":
			{
				addExpAndSp(player, EXP_REWARD, SP_REWARD);
				giveItems(player, ADENA);
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
		if ((qs.isCreated()) && player.isInCategory(CategoryType.FOURTH_CLASS_GROUP))
		{
			htmltext = "33648-01.htm";
		}
		else if (qs.isCond(1))
		{
			htmltext = "33648-05.html";
		}
		else if (qs.isCond(2))
		{
			htmltext = "33648-06.html";
		}
		else if (qs.isCompleted())
		{
			htmltext = "complete.htm";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if (qs != null)
		{
			switch (npc.getId())
			{
				case SUCCUBUS_SOLDIER:
				{
					int kills = qs.getInt(Integer.toString(SUCCUBUS_SOLDIER));
					if (kills < 12)
					{
						kills++;
						qs.set(Integer.toString(SUCCUBUS_SOLDIER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case SUCCUBUS_WARRIOR:
				{
					int kills = qs.getInt(Integer.toString(SUCCUBUS_WARRIOR));
					if (kills < 12)
					{
						kills++;
						qs.set(Integer.toString(SUCCUBUS_WARRIOR), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case SUCCUBUS_ARCHER:
				{
					int kills = qs.getInt(Integer.toString(SUCCUBUS_ARCHER));
					if (kills < 8)
					{
						kills++;
						qs.set(Integer.toString(SUCCUBUS_ARCHER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case SUCCUBUS_SHAMAN:
				{
					int kills = qs.getInt(Integer.toString(SUCCUBUS_SHAMAN));
					if (kills < 8)
					{
						kills++;
						qs.set(Integer.toString(SUCCUBUS_SHAMAN), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case BLOODY_SUCCUBUS:
				{
					int kills = qs.getInt(Integer.toString(BLOODY_SUCCUBUS));
					if (kills < 5)
					{
						kills++;
						qs.set(Integer.toString(BLOODY_SUCCUBUS), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(SUCCUBUS_SOLDIER, qs.getInt(Integer.toString(SUCCUBUS_SOLDIER)));
			log.addNpc(SUCCUBUS_WARRIOR, qs.getInt(Integer.toString(SUCCUBUS_WARRIOR)));
			log.addNpc(SUCCUBUS_ARCHER, qs.getInt(Integer.toString(SUCCUBUS_ARCHER)));
			log.addNpc(SUCCUBUS_SHAMAN, qs.getInt(Integer.toString(SUCCUBUS_SHAMAN)));
			log.addNpc(BLOODY_SUCCUBUS, qs.getInt(Integer.toString(BLOODY_SUCCUBUS)));
			qs.getPlayer().sendPacket(log);
			
			if ((qs.getInt(Integer.toString(SUCCUBUS_SOLDIER)) >= 12) && (qs.getInt(Integer.toString(SUCCUBUS_WARRIOR)) >= 12) && (qs.getInt(Integer.toString(SUCCUBUS_ARCHER)) >= 8) && (qs.getInt(Integer.toString(SUCCUBUS_SHAMAN)) >= 8) && (qs.getInt(Integer.toString(BLOODY_SUCCUBUS)) >= 5))
			{
				qs.setCond(2, true);
			}
		}
	}
}
