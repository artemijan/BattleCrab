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
package quests.Q00833_DevilsTreasureTauti;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.enums.Movie;

/**
 * Devil's Treasure, Tauti (833)
 * @URL https://l2wiki.com/Devil%27s_Treasure,_Tauti
 * @author Gigi
 */
public class Q00833_DevilsTreasureTauti extends Quest
{
	// NPCs
	private static final int DETON = 34170;
	private static final int SETTLEN = 34180;
	
	// Monsters
	private static final int FLAME_SCORPION = 23682;
	private static final int FLAME_GOLEM = 23680;
	private static final int FLAME_SCARAB = 23709;
	private static final int SEAL_TOMBSTONE = 19607;
	
	// Items
	private static final int FRENZED_TAUTIS_FRAGMENT = 47884;
	private static final int INSANE_KELBIMS_FRAGMENT = 47885;
	private static final int SOE_MISTYC_TAVERN = 46564;
	private static final int MYSTIC_ARMOR_PIACE = 46587;
	private static final int FLAME_FLOWER = 46554;
	
	public Q00833_DevilsTreasureTauti()
	{
		super(833);
		addStartNpc(DETON);
		addTalkId(SETTLEN);
		addKillId(FLAME_SCORPION, FLAME_GOLEM, FLAME_SCARAB, SEAL_TOMBSTONE);
		addCreatureSeeId(DETON);
		registerQuestItems(FLAME_FLOWER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		switch (event)
		{
			case "34180-02.html":
			{
				final int chance = getRandom(100);
				if (chance <= 10)
				{
					giveItems(player, FRENZED_TAUTIS_FRAGMENT, 1);
				}
				else if ((chance > 10) && (chance <= 20))
				{
					giveItems(player, INSANE_KELBIMS_FRAGMENT, 1);
				}
				else if ((chance > 20) && (chance <= 40))
				{
					giveItems(player, MYSTIC_ARMOR_PIACE, 1);
				}
				else
				{
					giveItems(player, SOE_MISTYC_TAVERN, 1);
				}
				
				addExpAndSp(player, 6_362_541_900L, 15_270_101);
				qs.exitQuest(QuestType.REPEATABLE, true);
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		String htmltext = getNoQuestMsg(player);
		final QuestState qs = getQuestState(player, true);
		
		switch (qs.getState())
		{
			case State.STARTED:
			{
				htmltext = (qs.isCond(8)) ? "34180-01.html" : "34180-03.html";
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		final Player player = creature.asPlayer();
		if (player != null)
		{
			final QuestState qs = getQuestState(player, true);
			if (!qs.isStarted())
			{
				playMovie(player, Movie.EPIC_TAUTI_SLIDE);
				qs.startQuest();
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
	
	private void onKill(Npc npc, Player killer)
	{
		final QuestState qs = getQuestState(killer, false);
		switch (npc.getId())
		{
			case FLAME_SCORPION:
			{
				if ((qs != null) && qs.isCond(1) && killer.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
				{
					int killedScorpion = qs.getInt("killed_" + FLAME_SCORPION);
					if (killedScorpion < 5)
					{
						qs.set("killed_" + FLAME_SCORPION, ++killedScorpion);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					
					if (killedScorpion >= 5)
					{
						qs.setCond(2, true);
					}
					
					sendNpcLogList(killer);
				}
				break;
			}
			case FLAME_GOLEM:
			{
				if ((qs != null) && qs.isCond(3) && killer.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
				{
					qs.setCond(5, true);
				}
				break;
			}
			case FLAME_SCARAB:
			{
				if ((qs != null) && qs.isCond(4) && killer.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
				{
					int killedScarab = qs.getInt("killed_" + FLAME_SCARAB);
					if (killedScarab < 5)
					{
						qs.set("killed_" + FLAME_SCARAB, ++killedScarab);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
					
					if (killedScarab >= 5)
					{
						qs.setCond(5, true);
					}
					
					sendNpcLogList(killer);
				}
				break;
			}
			case SEAL_TOMBSTONE:
			{
				if ((qs != null) && qs.isCond(5) && killer.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
				{
					qs.setCond(6, true);
				}
				break;
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && (qs.isCond(1) || qs.isCond(4)))
		{
			final Set<NpcLogListHolder> holder = new HashSet<>(2);
			holder.add(new NpcLogListHolder(FLAME_SCORPION, false, qs.getInt("killed_" + FLAME_SCORPION)));
			holder.add(new NpcLogListHolder(FLAME_SCARAB, false, qs.getInt("killed_" + FLAME_SCARAB)));
			return holder;
		}
		
		return super.getNpcLogList(player);
	}
}
