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
package quests.Q10304_ForForgottenHeroes;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLevelChanged;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerPressTutorialMark;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowQuestionMark;

import quests.Q10302_UnsettlingShadowAndRumors.Q10302_UnsettlingShadowAndRumors;

/**
 * For the Forgotten Heroes (10304)
 * @URL https://l2wiki.com/For_the_Forgotten_Heroes
 * @author Gigi
 */
public class Q10304_ForForgottenHeroes extends Quest
{
	// NPC
	private static final int ISHAEL = 32894;
	
	// Monsters
	private static final int YUI = 25837;
	private static final int KINEN = 25840;
	private static final int KONYAR = 25845;
	private static final int LESYINDA = 25841;
	private static final int MAKSHU = 25838;
	private static final int HORNAFI = 25839;
	private static final int YONTYMAK = 25846;
	private static final int RON = 25825;
	
	// Items
	private static final int EWR = 17526; // Scroll: Enchant Weapon (R-grade)
	private static final int EAR = 17527; // Scroll: Enchant Armor (R-grade)
	private static final int COKES = 36563; // Synthetic Cokes
	private static final int POUCH = 34861; // Ingredient and Hardener Pouch (R-grade)
	private static final int OLD_ROLL_OF_PAPER = 34033;
	
	// Misc
	private static final int MIN_LEVEL = 90;
	
	public Q10304_ForForgottenHeroes()
	{
		super(10304);
		addItemTalkId(OLD_ROLL_OF_PAPER);
		addTalkId(ISHAEL);
		addKillId(YUI, KINEN, KONYAR, LESYINDA, MAKSHU, HORNAFI, YONTYMAK, RON);
		addCondCompletedQuest(Q10302_UnsettlingShadowAndRumors.class.getSimpleName(), "32894-02.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "32894-02.htm":
			case "32894-03.htm":
			case "32894-08.html":
			{
				htmltext = event;
				break;
			}
			case "32894-06.htm":
			{
				qs.setCond(2);
				htmltext = event;
				break;
			}
			case "reward1":
			{
				giveItems(player, COKES, 68);
				addExpAndSp(player, 15197798, 3647);
				giveAdena(player, 47085998, true);
				qs.exitQuest(false, true);
				htmltext = "32894-09.html";
				break;
			}
			case "reward2":
			{
				giveItems(player, EWR, 1);
				giveItems(player, EAR, 1);
				addExpAndSp(player, 15197798, 3647);
				giveAdena(player, 47085998, true);
				qs.exitQuest(false, true);
				htmltext = "32894-09.html";
				break;
			}
			case "reward3":
			{
				giveItems(player, POUCH, 4);
				addExpAndSp(player, 15197798, 3647);
				giveAdena(player, 47085998, true);
				qs.exitQuest(false, true);
				htmltext = "32894-09.html";
				break;
			}
		}
		
		if (event.equalsIgnoreCase("condition"))
		{
			if ((player.getParty() == null))
			{
				return "32894-05.html";
			}
			else if ((player.getParty() != null) && (player.getParty().getLeader() != player))
			{
				return "32894-05.html";
			}
			else if ((player.getParty() != null) && (player.getParty().getLeader() == player) && (player.getParty().getMemberCount() < 7))
			{
				return "32894-05.html";
			}
			
			return "32894-04.htm";
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (qs.isCond(1))
		{
			htmltext = "32894-01.htm";
		}
		else if ((qs.getCond() > 1) && (qs.getCond() < 9))
		{
			htmltext = "32894-03.htm";
		}
		else if (qs.isCond(9))
		{
			htmltext = "32894-07.html";
		}
		else if (qs.isCompleted())
		{
			htmltext = "Complete.html";
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
		if (qs != null)
		{
			final int cond = qs.getCond();
			final int npcId = npc.getId();
			if (player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
			{
				if ((npcId == YUI) && (cond == 2))
				{
					qs.setCond(3, true);
				}
				else if ((npcId == KINEN) && (cond == 3))
				{
					qs.setCond(4, true);
				}
				else if ((npcId == KONYAR) && (cond == 4))
				{
					qs.setCond(5, true);
				}
				else if ((npcId == LESYINDA) && (cond == 5))
				{
					qs.setCond(6, true);
				}
				else if ((npcId == MAKSHU) && (cond == 6))
				{
					int kills = qs.getInt(Integer.toString(MAKSHU));
					if (kills < 1)
					{
						kills++;
						qs.set(Integer.toString(MAKSHU), kills);
					}
				}
				else if ((npcId == HORNAFI) && (cond == 6))
				{
					int kills = qs.getInt(Integer.toString(HORNAFI));
					if (kills < 1)
					{
						kills++;
						qs.set(Integer.toString(HORNAFI), kills);
					}
				}
				else if ((npcId == YONTYMAK) && (cond == 7))
				{
					qs.setCond(8);
				}
				else if ((npcId == RON) && (cond == 8))
				{
					qs.setCond(8);
				}
			}
			
			final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
			log.addNpc(MAKSHU, qs.getInt(Integer.toString(MAKSHU)));
			log.addNpc(HORNAFI, qs.getInt(Integer.toString(HORNAFI)));
			qs.getPlayer().sendPacket(log);
			if ((qs.getInt(Integer.toString(MAKSHU)) >= 1) && (qs.getInt(Integer.toString(HORNAFI)) >= 1))
			{
				qs.setCond(7, true);
			}
		}
	}
	
	@Override
	public String onItemTalk(Item item, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		final QuestState qs1 = player.getQuestState(Q10302_UnsettlingShadowAndRumors.class.getSimpleName());
		boolean startQuest = false;
		switch (qs.getState())
		{
			case State.CREATED:
			{
				startQuest = true;
				break;
			}
		}
		
		if (startQuest)
		{
			if ((player.getLevel() >= MIN_LEVEL) && qs1.isCompleted())
			{
				qs.startQuest();
				takeItems(player, OLD_ROLL_OF_PAPER, -1);
				htmltext = "";
			}
			else
			{
				htmltext = "32894-00.htm";
			}
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LEVEL_CHANGED)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLevelChanged(OnPlayerLevelChanged event)
	{
		final Player player = event.getPlayer();
		final QuestState qs = getQuestState(player, false);
		if ((qs == null) && (event.getOldLevel() < event.getNewLevel()) && canStartQuest(player) && (player.getLevel() >= MIN_LEVEL))
		{
			player.sendPacket(new TutorialShowQuestionMark(getId(), 1));
			playSound(player, QuestSound.ITEMSOUND_QUEST_TUTORIAL);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		final QuestState qs = getQuestState(player, false);
		if ((qs == null) && canStartQuest(player) && (player.getLevel() >= MIN_LEVEL))
		{
			player.sendPacket(new TutorialShowQuestionMark(getId(), 1));
			playSound(player, QuestSound.ITEMSOUND_QUEST_TUTORIAL);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_PRESS_TUTORIAL_MARK)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerPressTutorialMark(OnPlayerPressTutorialMark event)
	{
		final Player player = event.getPlayer();
		if ((event.getMarkId() == getId()) && canStartQuest(player) && (player.getLevel() >= MIN_LEVEL))
		{
			showOnScreenMsg(player, NpcStringId.YOU_CAN_USE_THE_OLD_ROLL_OF_PAPER_TO_BEGIN_THE_QUEST, ExShowScreenMessage.TOP_CENTER, 8000);
			if (!hasQuestItems(player, OLD_ROLL_OF_PAPER))
			{
				giveItems(player, OLD_ROLL_OF_PAPER, 1);
			}
		}
	}
}
