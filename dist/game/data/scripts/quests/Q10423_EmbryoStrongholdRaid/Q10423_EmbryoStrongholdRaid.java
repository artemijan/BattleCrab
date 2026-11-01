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
package quests.Q10423_EmbryoStrongholdRaid;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Embryo Stronghold Raid (10423)
 * @URL https://l2wiki.com/Embryo_Stronghold_Raid
 * @author Dmitri
 */
public class Q10423_EmbryoStrongholdRaid extends Quest
{
	// NPCs
	private static final int ERDA = 34319;
	
	// Monsters
	private static final int[] MOBS =
	{
		26199, // Sampson
		26200, // Hanson
		26201, // Grom
		26202, // Medvez
		26203, // Zigatan
		26204, // Hunchback Kwai
		26205, // Cornix
		26206, // Caranix
		26207, // Jonadan
		26208, // Demien
		26209, // Berg
		26210, // Tarku
		26211, // Tarpin
		26212, // Embryo Safe Vault
		26213, // Embryo Secret Vault
		26214, // Sakum
		26215, // Crazy Typhoon
		26216, // Cursed Haren
		26217, // Flynt
		26218, // Harp
		26219, // Maliss
		26220, // Isadora
		26221, // Whitra
		26222, // Bletra
		26223, // Upgraded Siege Tank
		26224, // Vegima
		26225, // Varonia
		26226, // Aronia
		26227, // Odd
		26228, // Even
		26229 // Nemertess
	};
	
	// Rewards
	private static final int SUPERIOR_GIANTS_CODEX = 46151; // Superior Giant's Codex - Mastery Chapter 1
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10423_EmbryoStrongholdRaid()
	{
		super(10423);
		addStartNpc(ERDA);
		addTalkId(ERDA);
		addKillId(MOBS);
		addCondMinLevel(MIN_LEVEL, "34319-00.htm");
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
			case "34319-02.htm":
			case "34319-03.htm":
			case "34319-07.html":
			{
				htmltext = event;
				break;
			}
			case "34319-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34319-08.html":
			{
				if (qs.isCond(2))
				{
					giveItems(player, SUPERIOR_GIANTS_CODEX, 1);
					addExpAndSp(player, 29682570651L, 71108570);
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
				htmltext = "34319-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34319-05.html";
				}
				else
				{
					htmltext = "34319-06.html";
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
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			int killedEmbryo = qs.getInt("killed_" + MOBS[0]);
			if (ArrayUtil.contains(MOBS, npc.getId()))
			{
				if (killedEmbryo < 30)
				{
					killedEmbryo++;
					qs.set("killed_" + MOBS[0], killedEmbryo);
					playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
				}
			}
			
			if (killedEmbryo == 30)
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isStarted() && qs.isCond(1))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>();
			holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_EMBRYO_OFFICER, qs.getInt("killed_" + MOBS[0])));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
