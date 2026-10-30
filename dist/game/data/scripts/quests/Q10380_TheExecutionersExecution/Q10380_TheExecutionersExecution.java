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
package quests.Q10380_TheExecutionersExecution;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExQuestNpcLogList;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;
import org.l2jmobius.gameserver.util.LocationUtil;

import quests.Q10379_AnUninvitedGuest.Q10379_AnUninvitedGuest;

/**
 * The Executioner's Execution (10380)
 * @URL https://l2wiki.com/The_Executioner%27s_Execution
 * @author Gigi
 */
public class Q10380_TheExecutionersExecution extends Quest
{
	// NPCs
	private static final int ENDRIGO = 30632;
	private static final int GUILLOTINE_OF_DEATH = 25892;
	private static final int NAGDU_THE_DEFORMED = 23201;
	private static final int SADIAC_THE_KILLER = 23199;
	private static final int ROSENIAS_DIVINE_SPIRIT = 23208;
	private static final int HASKAL_GHOST = 23205;
	private static final int CANTA_STANDING_BEAST = 23203;
	private static final int GAZAM = 23207;
	private static final int TURAN_GHOST = 23200;
	private static final int KILLER_FRANGS = 23204;
	private static final int KALLBERA = 23209;
	private static final int HAKAL_THE_BUTTCHERED = 23202;
	private static final int SAMMITA = 23206;
	
	// Item's
	private static final int GLORIOUS_T_SHIRT = 35291;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10380_TheExecutionersExecution()
	{
		super(10380);
		addStartNpc(ENDRIGO);
		addTalkId(ENDRIGO);
		addKillId(GUILLOTINE_OF_DEATH, NAGDU_THE_DEFORMED, SADIAC_THE_KILLER, ROSENIAS_DIVINE_SPIRIT, HASKAL_GHOST, CANTA_STANDING_BEAST, GAZAM, TURAN_GHOST, KILLER_FRANGS, KALLBERA, HAKAL_THE_BUTTCHERED, SAMMITA);
		addCondMinLevel(MIN_LEVEL, getNoQuestMsg(null));
		addCondCompletedQuest(Q10379_AnUninvitedGuest.class.getSimpleName(), "warden_endrigo_q10380_02.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "warden_endrigo_q10380_04.htm":
			case "warden_endrigo_q10380_05.htm":
			case "warden_endrigo_q10380_09.html":
			{
				htmltext = event;
				break;
			}
			case "warden_endrigo_q10380_06.htm":
			{
				qs.startQuest();
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, ENDRIGO, NpcStringId.IT_IS_TIME_TO_PUT_THIS_TO_AN_END_ARE_YOU_READY));
				htmltext = event;
				break;
			}
			case "warden_endrigo_q10380_10.html":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, ENDRIGO, NpcStringId.YOU_DEFEATED_THE_GUILLOTINE_OF_DEATH_I_THINK_THAT_WAS_TRULY_AMAZING));
				giveItems(player, GLORIOUS_T_SHIRT, 1);
				addExpAndSp(player, 1022967090, 245512);
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
		if (qs.isCreated())
		{
			htmltext = "warden_endrigo_q10380_01.htm";
		}
		else if (qs.isStarted())
		{
			if (qs.isCond(1))
			{
				htmltext = "warden_endrigo_q10380_07.html";
			}
			else if (qs.isCond(2))
			{
				htmltext = "warden_endrigo_q10380_08.html";
			}
		}
		else if (qs.isCompleted())
		{
			htmltext = "warden_endrigo_q10380_03.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && LocationUtil.checkIfInRange(PlayerConfig.ALT_PARTY_RANGE, npc, player, false) && qs.isCond(1))
		{
			switch (npc.getId())
			{
				case NAGDU_THE_DEFORMED:
				case SADIAC_THE_KILLER:
				case ROSENIAS_DIVINE_SPIRIT:
				case HASKAL_GHOST:
				case CANTA_STANDING_BEAST:
				case GAZAM:
				case TURAN_GHOST:
				case KILLER_FRANGS:
				case KALLBERA:
				case HAKAL_THE_BUTTCHERED:
				case SAMMITA:
				{
					if (getRandom(100) < 5)
					{
						showOnScreenMsg(player, NpcStringId.TO_DEFEAT_THE_GUILLOTINE_OF_DEATH_HOW_AMAZING, ExShowScreenMessage.TOP_CENTER, 8000);
						addSpawn(GUILLOTINE_OF_DEATH, npc.getX() + 500, npc.getY() + 500, npc.getZ(), 0, false, 180000);
					}
					break;
				}
				case GUILLOTINE_OF_DEATH:
				{
					int kills = qs.getInt(Integer.toString(GUILLOTINE_OF_DEATH));
					if (kills < 1)
					{
						kills++;
						qs.set(Integer.toString(GUILLOTINE_OF_DEATH), kills);
					}
					
					final ExQuestNpcLogList log = new ExQuestNpcLogList(getId());
					log.addNpc(GUILLOTINE_OF_DEATH, qs.getInt("GUILLOTINE_OF_DEATH"));
					player.sendPacket(log);
					
					if (qs.getInt(Integer.toString(GUILLOTINE_OF_DEATH)) == 1)
					{
						qs.setCond(2, true);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		executeForEachPlayer(killer, npc, isSummon, true, false);
	}
}
