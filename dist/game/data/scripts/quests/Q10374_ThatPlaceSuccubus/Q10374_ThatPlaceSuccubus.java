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
package quests.Q10374_ThatPlaceSuccubus;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.data.xml.CategoryData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * That Place Succubus (10374)
 * @URL https://l2wiki.com/That_Place_Succubus
 * @author Gigi
 */
public class Q10374_ThatPlaceSuccubus extends Quest
{
	// NPCs
	private static final int ANDREI = 31292;
	private static final int AGNES = 31588;
	private static final int ZENYA = 32140;
	
	// Items
	private static final ItemHolder ADENA = new ItemHolder(57, 500560);
	
	// Reward
	private static final int EXP_REWARD = 23747100;
	private static final int SP_REWARD = 5699;
	
	// Misc
	private static final int MIN_LEVEL = 80;
	
	// Monsters
	private static final int PHANTOM_SOLIDER = 23186;
	private static final int PHANTOM_WARRIOR = 23187;
	private static final int PHANTOM_ARCHER = 23188;
	private static final int PHANTOM_SHAMAN = 23189;
	private static final int PHANTOM_MARTYR = 23190;
	
	public Q10374_ThatPlaceSuccubus()
	{
		super(10374);
		addStartNpc(ANDREI);
		addTalkId(ANDREI, AGNES, ZENYA);
		addKillId(PHANTOM_SOLIDER, PHANTOM_WARRIOR, PHANTOM_ARCHER, PHANTOM_SHAMAN, PHANTOM_MARTYR);
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
			case "31292-02.htm":
			case "31292-04.html":
			case "31588-03.html":
			{
				htmltext = event;
				break;
			}
			case "31292-03.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "31588-02.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "32140-02.html":
			{
				qs.setCond(0);
				qs.setCond(3, true);
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
		if ((npc.getId() == ZENYA) && !CategoryData.getInstance().isInCategory(CategoryType.FOURTH_CLASS_GROUP, player.getPlayerClass().getId()))
		{
			return "noClass.html";
		}
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				switch (npc.getId())
				{
					case ANDREI:
					{
						htmltext = "31292-01.htm";
					}
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case ANDREI:
					{
						if (qs.isCond(1))
						{
							htmltext = "31292-04.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "31292-04.html";
						}
						break;
					}
					case AGNES:
					{
						if (qs.isCond(1))
						{
							htmltext = "31588-01.html";
						}
						else if (qs.isCond(2))
						{
							htmltext = "31588-03.html";
						}
						break;
					}
					case ZENYA:
					{
						if (qs.isCond(2))
						{
							htmltext = "32140-01.html";
						}
						else if (qs.isCond(3))
						{
							htmltext = "32140-04.html";
						}
						else if (qs.isCond(4))
						{
							giveItems(player, ADENA);
							addExpAndSp(player, EXP_REWARD, SP_REWARD);
							npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), NpcStringId.THANK_YOU_WITH_YOUR_HELP_WE_WERE_ABLE_TO_SURVIVE_A_GREAT_CRISIS));
							qs.exitQuest(false, true);
							htmltext = "32140-03.html";
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
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 3, 3, npc);
		if (qs != null)
		{
			switch (npc.getId())
			{
				case PHANTOM_SOLIDER:
				{
					int kills = qs.getInt(Integer.toString(PHANTOM_SOLIDER));
					if (kills < 15)
					{
						kills++;
						qs.set(Integer.toString(PHANTOM_SOLIDER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case PHANTOM_WARRIOR:
				{
					int kills = qs.getInt(Integer.toString(PHANTOM_WARRIOR));
					if (kills < 10)
					{
						kills++;
						qs.set(Integer.toString(PHANTOM_WARRIOR), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case PHANTOM_ARCHER:
				{
					int kills = qs.getInt(Integer.toString(PHANTOM_ARCHER));
					if (kills < 5)
					{
						kills++;
						qs.set(Integer.toString(PHANTOM_ARCHER), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case PHANTOM_SHAMAN:
				{
					int kills = qs.getInt(Integer.toString(PHANTOM_SHAMAN));
					if (kills < 5)
					{
						kills++;
						qs.set(Integer.toString(PHANTOM_SHAMAN), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
				case PHANTOM_MARTYR:
				{
					int kills = qs.getInt(Integer.toString(PHANTOM_MARTYR));
					if (kills < 5)
					{
						kills++;
						qs.set(Integer.toString(PHANTOM_MARTYR), kills);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					break;
				}
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(PHANTOM_SOLIDER, qs.getInt(Integer.toString(PHANTOM_SOLIDER)));
			log.addNpc(PHANTOM_WARRIOR, qs.getInt(Integer.toString(PHANTOM_WARRIOR)));
			log.addNpc(PHANTOM_ARCHER, qs.getInt(Integer.toString(PHANTOM_ARCHER)));
			log.addNpc(PHANTOM_SHAMAN, qs.getInt(Integer.toString(PHANTOM_SHAMAN)));
			log.addNpc(PHANTOM_MARTYR, qs.getInt(Integer.toString(PHANTOM_MARTYR)));
			qs.getPlayer().sendPacket(log);
			
			if ((qs.getInt(Integer.toString(PHANTOM_SOLIDER)) >= 15) && (qs.getInt(Integer.toString(PHANTOM_WARRIOR)) >= 10) && (qs.getInt(Integer.toString(PHANTOM_ARCHER)) >= 5) && (qs.getInt(Integer.toString(PHANTOM_SHAMAN)) >= 5) && (qs.getInt(Integer.toString(PHANTOM_MARTYR)) >= 5))
			{
				qs.setCond(4, true);
			}
		}
	}
}
