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
package quests.Q00667_HowToCoverShilensEyes;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * How to Cover Shilen's Eyes (667)
 * @author Kazumi
 */
public final class Q00667_HowToCoverShilensEyes extends Quest
{
	// NPCs
	private static final int ARCTURUS = 34267;
	private static final int COLIN = 30703;
	
	// Monster
	private static final int[] MONSTERS =
	{
		25915, // Anakim
		25919, // Lilith
	};
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int REWARD = 150;
	private static final int FACTION_AMITY_TOKEN = 48030;
	
	public Q00667_HowToCoverShilensEyes()
	{
		super(667);
		addStartNpc(ARCTURUS, COLIN);
		addTalkId(ARCTURUS, COLIN);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "");
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
		
		if ("quest_accept".equals(event))
		{
			qs.startQuest();
			switch (npc.getId())
			{
				case ARCTURUS:
				{
					htmltext = "hunter_leader_arcturus_q0667_05.htm";
					break;
				}
				case COLIN:
				{
					htmltext = "union_member_colin_q0667_05.htm";
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
				// fallthrou
			}
			case State.CREATED:
			{
				if (player.getLevel() >= MIN_LEVEL)
				{
					if (player.getFactionLevel(Faction.HUNTERS_GUILD) >= 1)
					{
						htmltext = npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_01.htm" : "union_member_colin_q0667_01.htm";
						break;
					}
					
					htmltext = npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_02.htm" : "union_member_colin_q0667_02.htm";
					break;
				}
				
				htmltext = npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_02.htm" : "union_member_colin_q0667_02.htm";
				break;
			}
			case State.STARTED:
			{
				switch (qs.getCond())
				{
					case 1:
					{
						htmltext = npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_06.htm" : "union_member_colin_q0667_06.htm";
						break;
					}
					case 2:
					{
						htmltext = npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_07.htm" : "union_member_colin_q0667_07.htm";
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(ARCTURUS)
	@Id(COLIN)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final QuestState qs = getQuestState(player, false);
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		
		if (ask == 667)
		{
			switch (reply)
			{
				case 1:
				{
					showHtmlFile(player, npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_03.htm" : "union_member_colin_q0667_03.htm");
					break;
				}
				case 2:
				{
					showHtmlFile(player, npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_04.htm" : "union_member_colin_q0667_04.htm");
					break;
				}
				case 1001:
				{
					if (qs.getCond() == 2)
					{
						qs.exitQuest(QuestType.DAILY, true);
						addFactionPoints(player, Faction.HUNTERS_GUILD, REWARD);
						showHtmlFile(player, npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_08.htm" : "union_member_colin_q0667_08.htm");
					}
					break;
				}
				case 1002:
				{
					if (qs.getCond() == 2)
					{
						if (hasQuestItems(player, FACTION_AMITY_TOKEN))
						{
							qs.exitQuest(QuestType.DAILY, true);
							takeItems(player, FACTION_AMITY_TOKEN, 1);
							addFactionPoints(player, Faction.HUNTERS_GUILD, REWARD * 2);
							showHtmlFile(player, npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_08.htm" : "union_member_colin_q0667_08.htm");
							break;
						}
						
						showHtmlFile(player, npc.getId() == ARCTURUS ? "hunter_leader_arcturus_q0667_09.htm" : "union_member_colin_q0667_09.htm");
					}
					break;
				}
			}
		}
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
			qs.setCond(2, true);
		}
	}
}
