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
package quests.Q10831_UnbelievableSight;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10830_TheLostGardenOfSpirits.Q10830_TheLostGardenOfSpirits;

/**
 * Unbelievable Sight (10831)
 * @URL https://l2wiki.com/Unbelievable_Sight
 * @author Gigi
 */
public class Q10831_UnbelievableSight extends Quest
{
	// NPC
	private static final int BELAS = 34056;
	
	// Monsters
	private static final int EL_FLOATO = 23559;
	private static final int KERBEROS_LAGER = 23550;
	private static final int KERBEROS_FORT = 23551;
	private static final int KERBEROS_NERO = 23552;
	private static final int FURY_SYLPH_BARRENA = 23553;
	private static final int FURY_SYLPH_TEMPTRESS = 23555;
	private static final int FURY_SYLPH_PURKA = 23556;
	private static final int FURY_KERBEROS_LEGER = 23557;
	private static final int FURY_KERBEROS_NERO = 23558;
	
	// Item's
	private static final int UNBALANCED_ENERGY = 45822;
	private static final int MARK_OF_TRUST_LOW_GRADE = 45840;
	private static final int SPIRIT_EVIL_TOUGHT_PIACE = 46130;
	private static final int SOE = 46158;
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10831_UnbelievableSight()
	{
		super(10831);
		addStartNpc(BELAS);
		addTalkId(BELAS);
		addKillId(EL_FLOATO);
		addKillId(KERBEROS_LAGER, KERBEROS_FORT, KERBEROS_NERO, FURY_SYLPH_BARRENA, FURY_SYLPH_TEMPTRESS, FURY_SYLPH_PURKA, FURY_KERBEROS_LEGER, FURY_KERBEROS_NERO);
		registerQuestItems(UNBALANCED_ENERGY);
		addCondMinLevel(MIN_LEVEL, "34056-00.htm");
		addCondCompletedQuest(Q10830_TheLostGardenOfSpirits.class.getSimpleName(), "34056-00.htm");
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
			case "34056-02.htm":
			case "34056-03.htm":
			{
				htmltext = event;
				break;
			}
			case "34056-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34056-07.html":
			{
				giveItems(player, SOE, 1);
				giveItems(player, SPIRIT_EVIL_TOUGHT_PIACE, 1);
				addExpAndSp(player, 1637472704L, 14237820);
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
		switch (qs.getState())
		{
			case State.CREATED:
			{
				if (!hasQuestItems(player, MARK_OF_TRUST_LOW_GRADE))
				{
					htmltext = "noItem.htm";
					break;
				}
				
				htmltext = "34056-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "34056-05.html";
				}
				else
				{
					htmltext = "34056-06.html";
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
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			switch (npc.getId())
			{
				case KERBEROS_LAGER:
				case KERBEROS_FORT:
				case KERBEROS_NERO:
				case FURY_SYLPH_BARRENA:
				case FURY_SYLPH_TEMPTRESS:
				case FURY_SYLPH_PURKA:
				case FURY_KERBEROS_LEGER:
				case FURY_KERBEROS_NERO:
				{
					final Npc mob = addSpawn(EL_FLOATO, npc.getX(), npc.getY(), npc.getZ(), 0, true, 120000);
					addAttackPlayerDesire(mob, killer, 5);
					break;
				}
				case EL_FLOATO:
				{
					if (giveItemRandomly(killer, npc, UNBALANCED_ENERGY, 1, 100, 0.5, true))
					{
						qs.setCond(2, true);
					}
					break;
				}
			}
		}
	}
}
