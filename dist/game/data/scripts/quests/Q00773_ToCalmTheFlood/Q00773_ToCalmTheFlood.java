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
package quests.Q00773_ToCalmTheFlood;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q00565_BasicMissionFairySettlementWest.Q00565_BasicMissionFairySettlementWest;
import quests.Q00749_TiesWithTheGuardians.Q00749_TiesWithTheGuardians;

/**
 * To Calm the Flood (773)
 * @author Kazumi
 */
public class Q00773_ToCalmTheFlood extends Quest
{
	// NPCs
	private static final int FAIRY = 32921;
	
	// Monster
	private static final int[] FAIRY_MONSTERS =
	{
		22863, // Fairy Warrior
		22867, // Fairy Warrior - Violent
		22868, // Fairy Warrior - Brutal
		22871, // Fairy Rogue
		22875, // Fairy Rogue - Violent
		22876, // Fairy Rogue - Brutal
		22879, // Fairy Knight
		22883, // Fairy Knight - Violent
		22884, // Fairy Knight - Brutal
	};
	private static final int[] SATIRE_MONSTERS =
	{
		22887, // Satyr Wizard
		22891, // Satyr Wizard - Violent
		22892, // Satyr Wizard - Brutal
		22895, // Satyr Summoner
		22899, // Satyr Summoner - Violent
		22900, // Satyr Summoner - Brutal
		22903, // Satyr Witch
		22907, // Satyr Witch - Violent
		22908, // Satyr Witch - Brutal
	};
	
	// Misc
	private static final int MIN_LEVEL = 88;
	private static final int MAX_LEVEL = 98;
	private static final int KILL_COUNT = 200;
	private static final int ID_FAIRY = 19705;
	private static final int ID_SATYR = 19706;
	
	public Q00773_ToCalmTheFlood()
	{
		super(773);
		addStartNpc(FAIRY);
		addTalkId(FAIRY);
		addKillId(FAIRY_MONSTERS);
		addKillId(SATIRE_MONSTERS);
		addCondMinLevel(MIN_LEVEL, "fairy_civilian_quest_q0773_02.htm");
		addCondMaxLevel(MAX_LEVEL, "fairy_civilian_quest_q0773_02.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = event;
		
		switch (event)
		{
			case "fairy_civilian_quest_q0773_03.htm":
			case "fairy_civilian_quest_q0773_04.htm":
			{
				htmltext = event;
				break;
			}
			case "fairy_civilian_quest_q0773_05.htm":
			{
				qs.startQuest();
				break;
			}
			case "fairy_civilian_quest_q0773_10.htm":
			{
				if (qs.isCond(2))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						qs.exitQuest(QuestType.DAILY, true);
						giveAdena(player, 1_448_604, true);
						addExpAndSp(player, 429526470, 429510);
						final Quest qs749 = ScriptManager.getInstance().getScript(Q00749_TiesWithTheGuardians.class.getSimpleName());
						if (qs749 != null)
						{
							qs749.notifyEvent("NOTIFY_Q749", npc, player);
						}
						
						final Quest qs565 = ScriptManager.getInstance().getScript(Q00565_BasicMissionFairySettlementWest.class.getSimpleName());
						if (qs565 != null)
						{
							qs565.notifyEvent("NOTIFY_Q773", npc, player);
						}
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
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				if (!qs.isNowAvailable())
				{
					htmltext = getAlreadyCompletedMsg(player);
					break;
				}
				
				qs.setState(State.CREATED);
				// fallthrough
			}
			case State.CREATED:
			{
				htmltext = "fairy_civilian_quest_q0773_01.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = "fairy_civilian_quest_q0773_06.htm";
						break;
					}
					case 2:
					{
						htmltext = "fairy_civilian_quest_q0773_07.htm";
						break;
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
		final Party party = killer.getParty();
		if (party != null)
		{
			party.getMembers().forEach(p -> onKill(npc, p));
		}
		else
		{
			onKill(npc, killer);
		}
	}
	
	public void onKill(Npc npc, Player killer)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && (npc.calculateDistance3D(killer) <= 1000))
		{
			int killedFairy = qs.getInt("killed_" + ID_FAIRY);
			int killedSatyr = qs.getInt("killed_" + ID_SATYR);
			
			if (ArrayUtil.contains(FAIRY_MONSTERS, npc.getId()))
			{
				if (killedFairy < KILL_COUNT)
				{
					qs.set("killed_" + ID_FAIRY, ++killedFairy);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				
				sendNpcLogList(killer);
			}
			
			if (ArrayUtil.contains(SATIRE_MONSTERS, npc.getId()))
			{
				if (killedSatyr < KILL_COUNT)
				{
					qs.set("killed_" + ID_SATYR, ++killedSatyr);
					playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
				
				sendNpcLogList(killer);
			}
			
			if ((killedFairy >= KILL_COUNT) && (killedSatyr >= KILL_COUNT))
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(ID_FAIRY, false, qs.getInt("killed_" + ID_FAIRY)));
			holder.add(new NpcLogListHolder(ID_SATYR, false, qs.getInt("killed_" + ID_SATYR)));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		sendNpcLogList(player);
	}
}
