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
package quests.Q10318_DecayingDarkness;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10317_OrbisWitch.Q10317_OrbisWitch;

/**
 * Decaying Darkness (10318)
 * @URL https://l2wiki.com/Decaying_Darkness_(quest)
 * @author Gigi
 */
public class Q10318_DecayingDarkness extends Quest
{
	// NPC
	private static final int LYDIA = 32892;
	
	// Summoners
	private static final int ORBIS_VICTIM = 22911;
	private static final int ORBIS_CURATOR = 22921;
	private static final int ORBIS_THROWER = 22917;
	private static final int ORBIS_ANCIENT_HERO = 22924;
	private static final int ORBIS_GUARD = 22915;
	private static final int ORBIS_CHIEF = 22927;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		18978, // Orbis' Victim Cursed
		18979, // Orbis' Guard Cursed
		18980, // Orbis' Thrower Cursed
		18981, // Orbis' Curator Cursed
		18982, // Orbis' Ancient Hero Cursed
		18983 // Orbis' Chief Curator Cursed
	};
	
	// Item
	private static final int CURSE_RESIDUE = 17733;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10318_DecayingDarkness()
	{
		super(10318);
		addStartNpc(LYDIA);
		addTalkId(LYDIA);
		addAttackId(ORBIS_VICTIM, ORBIS_CURATOR, ORBIS_THROWER, ORBIS_ANCIENT_HERO, ORBIS_GUARD, ORBIS_CHIEF);
		addKillId(MONSTERS);
		registerQuestItems(CURSE_RESIDUE);
		addCondMinLevel(MIN_LEVEL, "32892-09.html");
		addCondCompletedQuest(Q10317_OrbisWitch.class.getSimpleName(), "32892-09.html");
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
			case "32892-02.htm":
			case "32892-03.htm":
			case "32892-04.htm":
			{
				htmltext = event;
				break;
			}
			case "32892-05.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32892-07.html":
			{
				if (qs.isCond(2))
				{
					giveAdena(player, 5427900, false);
					addExpAndSp(player, 79260650, 19022);
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
		
		final int npcId = npc.getId();
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (npcId == LYDIA)
				{
					htmltext = "32892-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "32892-06.html";
				}
				else if (qs.isCond(2) && (getQuestItemsCount(player, CURSE_RESIDUE) >= 8))
				{
					htmltext = "32892-06a.html";
				}
				break;
			}
			case State.COMPLETED:
			{
				htmltext = "32892-08.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		final QuestState qs = getQuestState(attacker, false);
		if ((qs != null) && qs.isCond(1) && (getRandom(100) < 5))
		{
			switch (npc.getId())
			{
				case ORBIS_VICTIM:
				{
					final Npc mob = addSpawn(18978, npc.getX(), npc.getY(), npc.getZ(), 0, false, 60000);
					addAttackPlayerDesire(mob, attacker, 5);
					npc.deleteMe();
					break;
				}
				case ORBIS_CURATOR:
				{
					final Npc mob1 = addSpawn(18981, npc.getX(), npc.getY(), npc.getZ(), 0, false, 60000);
					addAttackPlayerDesire(mob1, attacker, 5);
					npc.deleteMe();
					break;
				}
				case ORBIS_THROWER:
				{
					final Npc mob2 = addSpawn(18980, npc.getX(), npc.getY(), npc.getZ(), 0, false, 60000);
					addAttackPlayerDesire(mob2, attacker, 5);
					npc.deleteMe();
					break;
				}
				case ORBIS_ANCIENT_HERO:
				{
					final Npc mob3 = addSpawn(18982, npc.getX(), npc.getY(), npc.getZ(), 0, false, 60000);
					addAttackPlayerDesire(mob3, attacker, 5);
					npc.deleteMe();
					break;
				}
				case ORBIS_GUARD:
				{
					final Npc mob4 = addSpawn(18979, npc.getX(), npc.getY(), npc.getZ(), 0, false, 60000);
					addAttackPlayerDesire(mob4, attacker, 5);
					npc.deleteMe();
					break;
				}
				case ORBIS_CHIEF:
				{
					final Npc mob5 = addSpawn(18983, npc.getX(), npc.getY(), npc.getZ(), 0, false, 60000);
					addAttackPlayerDesire(mob5, attacker, 5);
					npc.deleteMe();
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, -1, 3, npc);
		if ((qs != null) && qs.isCond(1) && giveItemRandomly(killer, CURSE_RESIDUE, 1, 8, 0.7, true))
		{
			qs.setCond(2, true);
		}
	}
}
