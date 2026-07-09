/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package quests.Q00416_PathOfTheOrcShaman;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.enums.ChatType;
import org.l2jmobius.gameserver.enums.ClassId;
import org.l2jmobius.gameserver.enums.QuestSound;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.holders.ItemChanceHolder;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;
import org.l2jmobius.gameserver.network.serverpackets.SocialAction;

/**
 * Path of the Orc Shaman (416)
 * @author Adry_85
 */
public class Q00416_PathOfTheOrcShaman extends Quest
{
	// NPCs
	private static final int UMOS = 30502;
	private static final int TATARU_ZU_HESTUI = 30585;
	private static final int HESTUI_TOTEM_SPIRIT = 30592;
	private static final int DUDA_MARA_TOTEM_SPIRIT = 30593;
	// Items
	private static final int FIRE_CHARM = 1616;
	private static final int KASHA_BEAR_PELT = 1617;
	private static final int KASHA_BLADE_SPIDER_HUSK = 1618;
	private static final int FIRST_FIERY_EGG = 1619;
	private static final int HESTUI_MASK = 1620;
	private static final int SECOND_FIERY_EGG = 1621;
	private static final int TOTEM_SPIRIT_CLAW = 1622;
	private static final int TATARUS_LETTER = 1623;
	private static final int FLAME_CHARM = 1624;
	private static final int GRIZZLY_BLOOD = 1625;
	private static final int BLOOD_CAULDRON = 1626;
	private static final int SPIRIT_NET = 1627;
	private static final int BOUND_DURKA_SPIRIT = 1628;
	private static final int DURKA_PARASITE = 1629;
	private static final int TOTEM_SPIRIT_BLOOD = 1630;
	private static final int MASK_OF_MEDIUM = 1631;
	// Quest Monsters
	private static final int DURKA_SPIRIT = 27056;
	private static final int BLACK_LEOPARD = 27319;
	// Misc
	private static final int MIN_LEVEL = 19;
	// Mobs
	private static final Map<Integer, ItemChanceHolder> MOBS = new HashMap<>();
	static
	{
		MOBS.put(20415, new ItemChanceHolder(FIRST_FIERY_EGG, 1, 1)); // scarlet_salamander
		MOBS.put(20478, new ItemChanceHolder(KASHA_BLADE_SPIDER_HUSK, 1, 1)); // kasha_blade_spider
		MOBS.put(20479, new ItemChanceHolder(KASHA_BEAR_PELT, 1, 1)); // kasha_bear
		MOBS.put(20335, new ItemChanceHolder(GRIZZLY_BLOOD, 1, 6)); // grizzly_bear
		MOBS.put(20038, new ItemChanceHolder(DURKA_PARASITE, 1, 9)); // poison_spider
		MOBS.put(20043, new ItemChanceHolder(DURKA_PARASITE, 1, 9)); // bind_poison_spider
		MOBS.put(27056, new ItemChanceHolder(DURKA_PARASITE, 1, 9)); // durka_spirit
	}
	
	public Q00416_PathOfTheOrcShaman()
	{
		super(416);
		addStartNpc(TATARU_ZU_HESTUI);
		addTalkId(TATARU_ZU_HESTUI, UMOS, DUDA_MARA_TOTEM_SPIRIT, HESTUI_TOTEM_SPIRIT);
		addKillId(MOBS.keySet());
		addKillId(BLACK_LEOPARD);
		registerQuestItems(FIRE_CHARM, KASHA_BEAR_PELT, KASHA_BLADE_SPIDER_HUSK, FIRST_FIERY_EGG, HESTUI_MASK, SECOND_FIERY_EGG, TOTEM_SPIRIT_CLAW, TATARUS_LETTER, FLAME_CHARM, GRIZZLY_BLOOD, BLOOD_CAULDRON, SPIRIT_NET, BOUND_DURKA_SPIRIT, DURKA_PARASITE, TOTEM_SPIRIT_BLOOD);
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
			case "START":
			{
				if (player.getClassId() != ClassId.ORC_MAGE)
				{
					if (player.getClassId() == ClassId.ORC_SHAMAN)
					{
						htmltext = "30585-02.htm";
					}
					else
					{
						htmltext = "30585-03.htm";
					}
				}
				else if (player.getLevel() < MIN_LEVEL)
				{
					htmltext = "30585-04.htm";
				}
				else if (hasQuestItems(player, MASK_OF_MEDIUM))
				{
					htmltext = "30585-05.htm";
				}
				else
				{
					htmltext = "30585-06.htm";
				}
				break;
			}
			case "30585-07.htm":
			{
				qs.startQuest();
				qs.setMemoState(1);
				giveItems(player, FIRE_CHARM, 1);
				htmltext = event;
				break;
			}
			case "30585-12.html":
			{
				if (hasQuestItems(player, TOTEM_SPIRIT_CLAW))
				{
					htmltext = event;
				}
				break;
			}
			case "30585-13.html":
			{
				if (hasQuestItems(player, TOTEM_SPIRIT_CLAW))
				{
					takeItems(player, TOTEM_SPIRIT_CLAW, -1);
					giveItems(player, TATARUS_LETTER, 1);
					qs.setCond(5, true);
					htmltext = event;
				}
				break;
			}
			case "30585-14.html":
			{
				if (hasQuestItems(player, TOTEM_SPIRIT_CLAW))
				{
					takeItems(player, TOTEM_SPIRIT_CLAW, -1);
					qs.setCond(12, true);
					qs.setMemoState(100);
					htmltext = event;
				}
				break;
			}
			case "30502-07.html":
			{
				if (hasQuestItems(player, TOTEM_SPIRIT_BLOOD))
				{
					takeItems(player, TOTEM_SPIRIT_BLOOD, -1);
					giveItems(player, MASK_OF_MEDIUM, 1);
					final int level = player.getLevel();
					if (level >= 20)
					{
						addExpAndSp(player, 80314, 5087);
					}
					else if (level >= 19)
					{
						addExpAndSp(player, 80314, 5087);
					}
					else
					{
						addExpAndSp(player, 80314, 5087);
					}
					qs.exitQuest(false, true);
					player.sendPacket(new SocialAction(player.getObjectId(), 3));
					htmltext = event;
				}
				break;
			}
			case "32090-05.html":
			{
				if (qs.isMemoState(106))
				{
					htmltext = event;
				}
				break;
			}
			case "32090-06.html":
			{
				if (qs.isMemoState(106))
				{
					qs.setMemoState(107);
					qs.setCond(18, true);
					htmltext = event;
				}
				break;
			}
			case "30593-02.html":
			{
				if (hasQuestItems(player, BLOOD_CAULDRON))
				{
					htmltext = event;
				}
				break;
			}
			case "30593-03.html":
			{
				if (hasQuestItems(player, BLOOD_CAULDRON))
				{
					takeItems(player, BLOOD_CAULDRON, -1);
					giveItems(player, SPIRIT_NET, 1);
					qs.setCond(9, true);
					htmltext = event;
				}
				break;
			}
			case "30592-02.html":
			{
				if (hasQuestItems(player, HESTUI_MASK, SECOND_FIERY_EGG))
				{
					htmltext = event;
				}
				break;
			}
			case "30592-03.html":
			{
				if (hasQuestItems(player, HESTUI_MASK, SECOND_FIERY_EGG))
				{
					takeItems(player, -1, HESTUI_MASK, SECOND_FIERY_EGG);
					giveItems(player, TOTEM_SPIRIT_CLAW, 1);
					qs.setCond(4, true);
					htmltext = event;
				}
				break;
			}
			case "32057-02.html":
			{
				if (qs.isMemoState(101))
				{
					qs.setMemoState(102);
					qs.setCond(14, true);
					htmltext = event;
				}
				break;
			}
			case "32057-05.html":
			{
				if (qs.isMemoState(109))
				{
					qs.setMemoState(110);
					qs.setCond(21, true);
					htmltext = event;
				}
				break;
			}
		}
		return htmltext;
	}
	
	@Override
	public String onKill(Npc npc, Player player, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(player, -1, 3, npc);
		if (qs == null)
		{
			return super.onKill(npc, player, isSummon);
		}
		
		if (npc.getId() == BLACK_LEOPARD)
		{
			switch (qs.getMemoState())
			{
				case 102:
				{
					qs.setMemoState(103);
					break;
				}
				case 103:
				{
					qs.setMemoState(104);
					qs.setCond(15, true);
					if (getRandom(100) < 66)
					{
						npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), NpcStringId.MY_DEAR_FRIEND_OF_S1_WHO_HAS_GONE_ON_AHEAD_OF_ME).addStringParameter(qs.getPlayer().getName()));
					}
					break;
				}
				case 105:
				{
					qs.setMemoState(106);
					qs.setCond(17, true);
					if (getRandom(100) < 66)
					{
						npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), NpcStringId.LISTEN_TO_TEJAKAR_GANDI_YOUNG_OROKA_THE_SPIRIT_OF_THE_SLAIN_LEOPARD_IS_CALLING_YOU_S1).addStringParameter(qs.getPlayer().getName()));
					}
					break;
				}
				case 107:
				{
					qs.setMemoState(108);
					qs.setCond(19, true);
					break;
				}
			}
			return super.onKill(npc, player, isSummon);
		}
		
		final ItemChanceHolder item = MOBS.get(npc.getId());
		if (item.getCount() == qs.getCond())
		{
			if (qs.isCond(1) && hasQuestItems(qs.getPlayer(), FIRE_CHARM))
			{
				if (giveItemRandomly(qs.getPlayer(), npc, item.getId(), 1, 1, item.getChance(), true) //
					&& hasQuestItems(qs.getPlayer(), FIRST_FIERY_EGG, KASHA_BLADE_SPIDER_HUSK, KASHA_BEAR_PELT))
				{
					qs.setCond(2, true);
				}
			}
			else if (qs.isCond(6) && hasQuestItems(qs.getPlayer(), FLAME_CHARM))
			{
				if (giveItemRandomly(qs.getPlayer(), npc, item.getId(), 1, 3, item.getChance(), true))
				{
					qs.setCond(7);
				}
			}
			else if (qs.isCond(9) && hasQuestItems(qs.getPlayer(), SPIRIT_NET) //
				&& !hasQuestItems(qs.getPlayer(), BOUND_DURKA_SPIRIT) //
				&& (getQuestItemsCount(qs.getPlayer(), DURKA_PARASITE) <= 8))
			{
				if ((npc.getId() == 20038) || (npc.getId() == 20043))
				{
					final int random = getRandom(10);
					final long itemCount = getQuestItemsCount(qs.getPlayer(), DURKA_PARASITE);
					if (((itemCount == 5) && (random < 1)) //
						|| ((itemCount == 6) && (random < 2)) //
						|| ((itemCount == 7) && (random < 2)) //
						|| (itemCount >= 8))
					{
						takeItems(player, DURKA_PARASITE, -1);
						addSpawn(DURKA_SPIRIT, npc.getX(), npc.getY(), npc.getZ(), 0, true, 0, false);
						playSound(qs.getPlayer(), QuestSound.ITEMSOUND_QUEST_BEFORE_BATTLE);
					}
					else
					{
						giveItems(qs.getPlayer(), DURKA_PARASITE, 1);
						playSound(qs.getPlayer(), QuestSound.ITEMSOUND_QUEST_ITEMGET);
					}
				}
				else
				{
					giveItems(qs.getPlayer(), BOUND_DURKA_SPIRIT, 1);
					takeItems(qs.getPlayer(), -1, DURKA_PARASITE, SPIRIT_NET);
				}
			}
		}
		return super.onKill(npc, player, isSummon);
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		if (qs.isCreated())
		{
			if (npc.getId() == TATARU_ZU_HESTUI)
			{
				htmltext = "30585-01.htm";
			}
		}
		else if (qs.isCompleted())
		{
			if (npc.getId() == TATARU_ZU_HESTUI)
			{
				return htmltext;
			}
		}
		else if (qs.isStarted())
		{
			switch (npc.getId())
			{
				case TATARU_ZU_HESTUI:
				{
					if (qs.isMemoState(1))
					{
						if (hasQuestItems(player, FIRE_CHARM))
						{
							if (getQuestItemsCount(player, KASHA_BEAR_PELT, KASHA_BLADE_SPIDER_HUSK, FIRST_FIERY_EGG) < 3)
							{
								htmltext = "30585-08.html";
							}
							else
							{
								takeItems(player, -1, FIRE_CHARM, KASHA_BEAR_PELT, KASHA_BLADE_SPIDER_HUSK, FIRST_FIERY_EGG);
								giveItems(player, HESTUI_MASK, 1);
								giveItems(player, SECOND_FIERY_EGG, 1);
								qs.setCond(3, true);
								htmltext = "30585-09.html";
							}
						}
						else if (hasQuestItems(player, HESTUI_MASK, SECOND_FIERY_EGG))
						{
							htmltext = "30585-10.html";
						}
						else if (hasQuestItems(player, TOTEM_SPIRIT_CLAW))
						{
							htmltext = "30585-11.html";
						}
						else if (hasQuestItems(player, TATARUS_LETTER))
						{
							htmltext = "30585-15.html";
						}
						else if (hasAtLeastOneQuestItem(player, GRIZZLY_BLOOD, FLAME_CHARM, BLOOD_CAULDRON, SPIRIT_NET, BOUND_DURKA_SPIRIT, TOTEM_SPIRIT_BLOOD))
						{
							htmltext = "30585-16.html";
						}
					}
					else if (qs.isMemoState(100))
					{
						htmltext = "30585-14.html";
					}
					break;
				}
				case UMOS:
				{
					if (qs.isMemoState(1))
					{
						if (hasQuestItems(player, TATARUS_LETTER))
						{
							giveItems(player, FLAME_CHARM, 1);
							takeItems(player, TATARUS_LETTER, -1);
							qs.setCond(6, true);
							htmltext = "30502-01.html";
						}
						else if (hasQuestItems(player, FLAME_CHARM))
						{
							if (getQuestItemsCount(player, GRIZZLY_BLOOD) < 3)
							{
								htmltext = "30502-02.html";
							}
							else
							{
								takeItems(player, -1, FLAME_CHARM, GRIZZLY_BLOOD);
								giveItems(player, BLOOD_CAULDRON, 1);
								qs.setCond(8, true);
								htmltext = "30502-03.html";
							}
						}
						else if (hasQuestItems(player, BLOOD_CAULDRON))
						{
							htmltext = "30502-04.html";
						}
						else if (hasAtLeastOneQuestItem(player, BOUND_DURKA_SPIRIT, SPIRIT_NET))
						{
							htmltext = "30502-05.html";
						}
						else if (hasQuestItems(player, TOTEM_SPIRIT_BLOOD))
						{
							htmltext = "30502-06.html";
						}
					}
					break;
				}
				case DUDA_MARA_TOTEM_SPIRIT:
				{
					if (qs.isMemoState(1))
					{
						if (hasQuestItems(player, BLOOD_CAULDRON))
						{
							htmltext = "30593-01.html";
						}
						else if (hasQuestItems(player, SPIRIT_NET) && !hasQuestItems(player, BOUND_DURKA_SPIRIT))
						{
							htmltext = "30593-04.html";
						}
						else if (!hasQuestItems(player, SPIRIT_NET) && hasQuestItems(player, BOUND_DURKA_SPIRIT))
						{
							takeItems(player, BOUND_DURKA_SPIRIT, -1);
							giveItems(player, TOTEM_SPIRIT_BLOOD, 1);
							qs.setCond(11, true);
							htmltext = "30593-05.html";
						}
						else if (hasQuestItems(player, TOTEM_SPIRIT_BLOOD))
						{
							htmltext = "30593-06.html";
						}
					}
					break;
				}
				case HESTUI_TOTEM_SPIRIT:
				{
					if (qs.isMemoState(1))
					{
						if (hasQuestItems(player, HESTUI_MASK, SECOND_FIERY_EGG))
						{
							htmltext = "30592-01.html";
						}
						else if (hasQuestItems(player, TOTEM_SPIRIT_CLAW))
						{
							htmltext = "30592-04.html";
						}
						else if (hasAtLeastOneQuestItem(player, GRIZZLY_BLOOD, FLAME_CHARM, BLOOD_CAULDRON, SPIRIT_NET, BOUND_DURKA_SPIRIT, TOTEM_SPIRIT_BLOOD, TATARUS_LETTER))
						{
							htmltext = "30592-05.html";
						}
					}
					break;
				}
			}
		}
		return htmltext;
	}
}
