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
package quests.Q00457_LostAndFound;

import java.util.Set;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.data.SpawnTable;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.spawns.Spawn;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.CreatureSay;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * Lost and Found (457)
 * @author nonom
 */
public class Q00457_LostAndFound extends Quest
{
	// NPCs
	private static final int GUMIEL = 32759;
	private static final int ESCORT_CHECKER = 32764;
	private static final int[] SOLINA_CLAN =
	{
		22789, // Guide Solina
		22790, // Seeker Solina
		22791, // Savior Solina
		22793, // Ascetic Solina
	};
	
	// Misc
	private static final int PACKAGED_BOOK = 15716;
	private static final int CHANCE_SPAWN = 1; // 1%
	private static final int MIN_LV = 82;
	private static Set<Spawn> _escortCheckers;
	
	public Q00457_LostAndFound()
	{
		super(457);
		addStartNpc(GUMIEL);
		addSpawnId(ESCORT_CHECKER);
		addFirstTalkId(GUMIEL);
		addTalkId(GUMIEL);
		addKillId(SOLINA_CLAN);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		String htmltext = null;
		switch (event)
		{
			case "32759-06.html":
			{
				npc.setScriptValue(0);
				qs.startQuest();
				npc.setTarget(player);
				npc.setWalking();
				npc.getAI().setIntention(Intention.FOLLOW, player);
				startQuestTimer("CHECK", 1000, npc, player, true);
				startQuestTimer("TIME_LIMIT", 600000, npc, player);
				startQuestTimer("TALK_TIME", 120000, npc, player);
				startQuestTimer("TALK_TIME2", 30000, npc, player);
				break;
			}
			case "TALK_TIME":
			{
				broadcastNpcSay(npc, player, NpcStringId.AH_I_THINK_I_REMEMBER_THIS_PLACE, false);
				break;
			}
			case "TALK_TIME2":
			{
				broadcastNpcSay(npc, player, NpcStringId.WHAT_WERE_YOU_DOING_HERE, false);
				startQuestTimer("TALK_TIME3", 10 * 1000, npc, player);
				break;
			}
			case "TALK_TIME3":
			{
				broadcastNpcSay(npc, player, NpcStringId.I_GUESS_YOU_RE_THE_SILENT_TYPE_THEN_ARE_YOU_LOOKING_FOR_TREASURE_LIKE_ME, false);
				break;
			}
			case "TIME_LIMIT":
			{
				startQuestTimer("STOP", 2000, npc, player);
				qs.exitQuest(QuestType.DAILY);
				break;
			}
			case "CHECK":
			{
				final double distance = npc.calculateDistance3D(player);
				if (distance > 1000)
				{
					if (distance > 5000)
					{
						startQuestTimer("STOP", 2000, npc, player);
						qs.exitQuest(QuestType.DAILY);
					}
					else if (npc.isScriptValue(0))
					{
						broadcastNpcSay(npc, player, NpcStringId.HEY_DON_T_GO_SO_FAST, true);
						npc.setScriptValue(1);
					}
					else if (npc.isScriptValue(1))
					{
						broadcastNpcSay(npc, player, NpcStringId.IT_S_HARD_TO_FOLLOW, true);
						npc.setScriptValue(2);
					}
					else if (npc.isScriptValue(2))
					{
						startQuestTimer("STOP", 2000, npc, player);
						qs.exitQuest(QuestType.DAILY);
					}
				}
				
				for (Spawn escortSpawn : _escortCheckers)
				{
					final Npc escort = escortSpawn.getLastSpawn();
					if ((escort != null) && npc.isInsideRadius2D(escort, 1000))
					{
						startQuestTimer("STOP", 1000, npc, player);
						startQuestTimer("BYE", 3000, npc, player);
						cancelQuestTimer("CHECK", npc, player);
						npc.broadcastPacket(new CreatureSay(npc, ChatType.NPC_GENERAL, NpcStringId.AH_FRESH_AIR));
						broadcastNpcSay(npc, player, NpcStringId.AH_FRESH_AIR, false);
						giveItems(player, PACKAGED_BOOK, 1);
						qs.exitQuest(QuestType.DAILY, true);
						break;
					}
				}
				break;
			}
			case "STOP":
			{
				npc.setTarget(null);
				npc.getAI().stopFollow();
				npc.getAI().setIntention(Intention.IDLE);
				cancelQuestTimer("CHECK", npc, player);
				cancelQuestTimer("TIME_LIMIT", npc, player);
				cancelQuestTimer("TALK_TIME", npc, player);
				cancelQuestTimer("TALK_TIME2", npc, player);
				break;
			}
			case "BYE":
			{
				npc.deleteMe();
				break;
			}
			default:
			{
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		if (npc.getTarget() != null)
		{
			return npc.getTarget().equals(player) ? "32759-08.html" : "32759-01a.html";
		}
		
		return "32759.html";
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, true);
		if ((getRandom(100) < CHANCE_SPAWN) && qs.isNowAvailable() && (player.getLevel() >= MIN_LV))
		{
			addSpawn(GUMIEL, npc);
		}
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
				htmltext = (player.getLevel() >= MIN_LV) ? "32759-01.htm" : "32759-03.html";
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					htmltext = (player.getLevel() >= MIN_LV) ? "32759-01.htm" : "32759-03.html";
				}
				else
				{
					htmltext = "32759-02.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		_escortCheckers = SpawnTable.getInstance().getSpawns(ESCORT_CHECKER);
	}
	
	private void broadcastNpcSay(Npc npc, Player player, NpcStringId stringId, boolean whisper)
	{
		(whisper ? player : npc).sendPacket(new NpcSay(npc.getObjectId(), (whisper ? ChatType.NPC_WHISPER : ChatType.NPC_GENERAL), npc.getId(), stringId));
	}
}
