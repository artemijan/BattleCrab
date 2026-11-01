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
package quests.Q10535_BlacksmithsSoul3;

import java.util.List;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10356_BlacksmithsSoul2.Q10356_BlacksmithsSoul2;

/**
 * @author Sero
 */
public class Q10535_BlacksmithsSoul3 extends Quest
{
	// NPCs
	private static final int BLACKSMITH_MAMMON = 31126;
	private static final int SHADAI = 32347;
	private static final int ISHUMA = 32615;
	
	// Monsters
	private static final int[] HELL_MONSTERS =
	{
		23386, // Jabberwok
		23387, // Kanzaroth
		23388, // Kandiloth
		23384, // Smaug
		23385, // Lunatikan
		23399, // Bend Beetle
		23398, // Koraza
		23397, // Desert Wendigo
		23395, // Garion
		23396, // Garion neti
	};
	private static final int[] CAVE_MONSTERS =
	{
		23727, // Shaqrima Bathus
		23728, // Shaqrima Carcass
		23729, // Kshana
	};
	
	// Items
	private static final int OREWITH_GIANTS_ENERGY = 47892;
	private static final int CRYSTAL_WITH_MAGOCAL_POWER = 47891;
	private static final int ENCHANTED_SHADOW_INGOT = 47886;
	
	// Misc
	private static final int MAIN_LEVEL = 99;
	
	public Q10535_BlacksmithsSoul3()
	{
		super(10535);
		addStartNpc(BLACKSMITH_MAMMON);
		addTalkId(BLACKSMITH_MAMMON, SHADAI, ISHUMA);
		addKillId(HELL_MONSTERS);
		addKillId(CAVE_MONSTERS);
		addCondCompletedQuest(Q10356_BlacksmithsSoul2.class.getSimpleName(), "31126-02.htm");
		addCondMinLevel(MAIN_LEVEL, getNoQuestMsg(null));
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (npc.getId())
		{
			case BLACKSMITH_MAMMON:
			{
				switch (qs.getState())
				{
					case State.CREATED:
					{
						htmltext = "31126-00.htm";
						qs.startQuest();
						break;
					}
					case State.STARTED:
					{
						if (qs.isCond(9))
						{
							htmltext = "31126-01.htm";
							addExpAndSp(player, 40346120829L, 36311508);
							giveItems(player, ENCHANTED_SHADOW_INGOT, 1);
							qs.exitQuest(false, true);
						}
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "31126-00a.htm";
							break;
						}
					}
				}
				break;
			}
			case SHADAI:
			{
				switch (qs.getState())
				{
					case State.STARTED:
					{
						if (qs.isCond(7) && (getQuestItemsCount(player, CRYSTAL_WITH_MAGOCAL_POWER) >= 500) && (getQuestItemsCount(player, OREWITH_GIANTS_ENERGY) >= 500))
						{
							htmltext = "32347-00.htm";
							takeItems(player, CRYSTAL_WITH_MAGOCAL_POWER, -1);
							qs.setCond(8);
						}
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "31126-00a.htm";
							break;
						}
					}
				}
				break;
			}
			case ISHUMA:
			{
				switch (qs.getState())
				{
					case State.STARTED:
					{
						if (qs.isCond(8) && (getQuestItemsCount(player, OREWITH_GIANTS_ENERGY) >= 500))
						{
							htmltext = "32615-00.htm";
							takeItems(player, OREWITH_GIANTS_ENERGY, -1);
							qs.setCond(9);
						}
						break;
					}
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = "31126-00a.htm";
							break;
						}
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
		if (killer.isInParty())
		{
			final Party party = killer.getParty();
			final List<Player> partyMember = party.getMembers();
			for (Player singleMember : partyMember)
			{
				final QuestState qsPartyMember = getQuestState(singleMember, false);
				final double distance = npc.calculateDistance3D(singleMember);
				if ((qsPartyMember != null) && (distance <= 1000))
				{
					if (qsPartyMember.isCond(1))
					{
						if (ArrayUtil.contains(HELL_MONSTERS, npc.getId()) && (getQuestItemsCount(singleMember, CRYSTAL_WITH_MAGOCAL_POWER) < 500))
						{
							giveItems(singleMember, CRYSTAL_WITH_MAGOCAL_POWER, 1);
							playSound(singleMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						}
					}
					
					if (qsPartyMember.isCond(1))
					{
						if (ArrayUtil.contains(CAVE_MONSTERS, npc.getId()) && (getQuestItemsCount(singleMember, OREWITH_GIANTS_ENERGY) < 500))
						{
							giveItems(singleMember, OREWITH_GIANTS_ENERGY, 1);
							playSound(singleMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						}
					}
					
					if (qsPartyMember.isCond(1) && (getQuestItemsCount(singleMember, OREWITH_GIANTS_ENERGY) >= 500) && (getQuestItemsCount(singleMember, CRYSTAL_WITH_MAGOCAL_POWER) >= 500))
					{
						qsPartyMember.setCond(7);
					}
				}
			}
		}
		else
		{
			final QuestState qs = getRandomPartyMemberState(killer, -1, 3, npc);
			if (qs != null)
			{
				if (qs.isCond(1))
				{
					if (ArrayUtil.contains(HELL_MONSTERS, npc.getId()) && (getQuestItemsCount(killer, CRYSTAL_WITH_MAGOCAL_POWER) < 500))
					{
						giveItems(killer, CRYSTAL_WITH_MAGOCAL_POWER, 1);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
						
					}
				}
				
				if (qs.isCond(1))
				{
					if (ArrayUtil.contains(CAVE_MONSTERS, npc.getId()) && (getQuestItemsCount(killer, OREWITH_GIANTS_ENERGY) < 500))
					{
						giveItems(killer, OREWITH_GIANTS_ENERGY, 1);
						playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
				}
				
				if (qs.isCond(1) && (getQuestItemsCount(killer, OREWITH_GIANTS_ENERGY) >= 500) && (getQuestItemsCount(killer, CRYSTAL_WITH_MAGOCAL_POWER) >= 500))
				{
					qs.setCond(7);
				}
			}
		}
	}
}
