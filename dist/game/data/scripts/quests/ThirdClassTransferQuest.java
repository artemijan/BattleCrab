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
package quests;

import java.util.EnumMap;
import java.util.Map;
import java.util.Map.Entry;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLevelChanged;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowQuestionMark;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Abstract class for all Third Class Transfer quests.
 * @author St3eT, Trevor The Third
 */
public abstract class ThirdClassTransferQuest extends Quest
{
	// NPCs
	private static final int QUARTERMASTER = 33407;
	private static final int VANGUARD_MEMBER = 33165;
	private static final int[] VANGUARDS =
	{
		33166,
		33167,
		33168,
		33169,
	};
	
	// Items
	private static final Map<Race, Integer> RACE_TAGS = new EnumMap<>(Race.class);
	static
	{
		RACE_TAGS.put(Race.HUMAN, 17748);
		RACE_TAGS.put(Race.ELF, 17749);
		RACE_TAGS.put(Race.DARK_ELF, 17750);
		RACE_TAGS.put(Race.ORC, 17751);
		RACE_TAGS.put(Race.DWARF, 17752);
		RACE_TAGS.put(Race.KAMAEL, 17753);
	}
	private static final int SOULSHOTS = 1467;
	private static final int SPIRITSHOTS = 3952;
	private static final int BLESSED_SCROLL_OF_RESURRECTION = 33518;
	private static final int PAULINAS_EQUIPMENT_SET = 46852;
	
	// Skills
	private static final SkillHolder SHOW_SKILL = new SkillHolder(5103, 1);
	
	// Misc
	private static final int QUESTION_MARK_ID = 101;
	private final int _minLevel;
	private final Race _race;
	
	public ThirdClassTransferQuest(int questId, int minLevel, Race race)
	{
		super(questId);
		addTalkId(QUARTERMASTER, VANGUARD_MEMBER);
		addTalkId(VANGUARDS);
		for (Entry<Race, Integer> tag : RACE_TAGS.entrySet())
		{
			registerQuestItems(tag.getValue());
		}
		// @formatter:off
		registerQuestItems(
			17484, // Cry of Destiny - Gladiator
			17485, // Cry of Destiny - Warlord
			17486, // Cry of Destiny - Paladin
			17487, // Cry of Destiny - Dark Avanger
			17488, // Cry of Destiny - Treasure Hunter
			17489, // Cry of Destiny - Hawkeye
			17490, // Cry of Destiny - Sorcerer
			17491, // Cry of Destiny - Necromancer
			17492, // Cry of Destiny - Warlock
			17493, // Cry of Destiny - Bishop
			17494, // Cry of Destiny - Prophet
			17495, // Cry of Destiny - Temple Knight
			17496, // Cry of Destiny - Swordsinger
			17497, // Cry of Destiny - Plains Walker
			17498, // Cry of Destiny - Silver Ranger
			17499, // Cry of Destiny - Spellsinger
			17500, // Cry of Destiny - Elemental Summoner
			17501, // Cry of Destiny - Elder
			17502, // Cry of Destiny - Shillien Knight
			17503, // Cry of Destiny - Bladecancer
			17504, // Cry of Destiny - Abyss Walker
			17505, // Cry of Destiny - Phantom Ranger
			17506, // Cry of Destiny - Spellhower
			17507, // Cry of Destiny - Phantom Summoner
			17508, // Cry of Destiny - Shillen Elder
			17509, // Cry of Destiny - Destroyer
			17510, // Cry of Destiny - Tyrant
			17511, // Cry of Destiny - Overlord
			17512, // Cry of Destiny - Warcryer
			17513, // Cry of Destiny - Bounty Hunter
			17514, // Cry of Destiny - Warsmith
			17515, // Cry of Destiny - Berserker
			17516, // Cry of Destiny - Soulbreaker (male)
			17516, // Cry of Destiny - Soulbreaker (female)
			17517 // Cry of Destiny - Arbalester
		);
		// @formatter:on
		_minLevel = minLevel;
		_race = race;
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
			case "33407-02.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
					htmltext = event;
				}
				break;
			}
			case "33407-05.html":
			{
				if (qs.isCond(3))
				{
					qs.setCond(4, true);
					qs.unset("vanguard");
					takeItems(player, RACE_TAGS.get(player.getRace()).intValue(), -1);
					htmltext = event;
				}
				break;
			}
			case "33165-02.html":
			{
				if (qs.isCond(4))
				{
					qs.setCond(5, true);
					htmltext = event;
				}
				break;
			}
			case "collectTag":
			{
				if (qs.isCond(2))
				{
					final int bit = 1 << (VANGUARDS[0] - npc.getId());
					final int vanguard = qs.getInt("vanguard");
					if ((vanguard & bit) != bit)
					{
						giveItems(player, RACE_TAGS.get(player.getRace()).intValue(), 1);
						qs.set("vanguard", vanguard | bit);
						if (getQuestItemsCount(player, RACE_TAGS.get(player.getRace()).intValue()) == 4)
						{
							qs.setCond(3, true);
							htmltext = "vanguard-04.html";
						}
						else
						{
							htmltext = "vanguard-02.html";
						}
					}
					else
					{
						htmltext = "vanguard-03.html";
					}
				}
				break;
			}
			case "nextClassInfo":
			{
				if ((qs.getInt("STARTED_CLASS") != player.getPlayerClass().getId()) && (player.getLevel() >= _minLevel))
				{
					htmltext = npc.getId() + "-10.html";
					break;
				}
				
				final PlayerClass newClassId = player.getPlayerClass().getNextClasses().stream().findFirst().orElse(null);
				if (newClassId != null)
				{
					htmltext = "class_preview_" + newClassId.toString().toLowerCase() + ".html";
				}
				break;
			}
			case "classTransfer":
			{
				if ((qs.getInt("STARTED_CLASS") != player.getPlayerClass().getId()) && (player.getLevel() >= _minLevel))
				{
					htmltext = npc.getId() + "-10.html";
					break;
				}
				
				final PlayerClass newClassId = player.getPlayerClass().getNextClasses().stream().findFirst().orElse(null);
				if (newClassId != null)
				{
					final PlayerClass currentPlayerClass = player.getPlayerClass();
					if (!newClassId.childOf(currentPlayerClass))
					{
						break;
					}
					
					addSkillCastDesire(npc, player, SHOW_SKILL.getSkill(), 23);
					player.sendPacket(SystemMessageId.CONGRATULATIONS_YOU_VE_COMPLETED_YOUR_THIRD_CLASS_TRANSFER_QUEST);
					player.broadcastSocialAction(3);
					if (!player.isSubClassActive())
					{
						player.setBaseClass(newClassId);
					}
					
					player.setPlayerClass(newClassId.getId());
					player.store(false);
					player.broadcastUserInfo();
					player.sendSkillList();
					giveItems(player, SOULSHOTS, 8000);
					giveItems(player, SPIRITSHOTS, 8000);
					giveItems(player, BLESSED_SCROLL_OF_RESURRECTION, 3);
					giveItems(player, PAULINAS_EQUIPMENT_SET, 1);
					addExpAndSp(player, 42000000, 0);
					qs.exitQuest(true, true);
					htmltext = npc.getId() + "-09.html";
				}
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
		if (qs.getState() == State.STARTED)
		{
			switch (npc.getId())
			{
				case QUARTERMASTER:
				{
					switch (qs.getCond())
					{
						case 1:
						{
							htmltext = "33407-01.html";
							break;
						}
						case 2:
						{
							htmltext = "33407-03.html";
							break;
						}
						case 3:
						{
							htmltext = "33407-04.html";
							break;
						}
						case 4:
						case 5:
						case 6:
						case 7:
						case 8:
						case 9:
						case 10:
						case 11:
						case 12:
						{
							htmltext = "33407-05.html";
							break;
						}
					}
					break;
				}
				case VANGUARD_MEMBER:
				{
					switch (qs.getCond())
					{
						case 4:
						case 5:
						case 6:
						case 7:
						case 8:
						case 9:
						case 10:
						case 11:
						case 12:
						{
							htmltext = "33165-01.html";
							break;
						}
					}
					break;
				}
				default:
				{
					if (qs.isCond(2) && ArrayUtil.contains(VANGUARDS, npc.getId()))
					{
						htmltext = "vanguard-01.html";
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LEVEL_CHANGED)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLevelChanged(OnPlayerLevelChanged event)
	{
		if (PlayerConfig.DISABLE_TUTORIAL)
		{
			return;
		}
		
		final Player player = event.getPlayer();
		final int oldLevel = event.getOldLevel();
		final int newLevel = event.getNewLevel();
		if ((oldLevel < newLevel) && (newLevel == _minLevel) && (player.getRace() == _race) && (player.isInCategory(CategoryType.THIRD_CLASS_GROUP)))
		{
			player.sendPacket(new TutorialShowQuestionMark(QUESTION_MARK_ID, 1));
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLogin(OnPlayerLogin event)
	{
		if (PlayerConfig.DISABLE_TUTORIAL)
		{
			return;
		}
		
		final Player player = event.getPlayer();
		if ((player.getLevel() >= _minLevel) && (player.getRace() == _race) && (player.isInCategory(CategoryType.THIRD_CLASS_GROUP)))
		{
			final QuestState qs = getQuestState(player, true);
			if (qs.isCreated())
			{
				player.sendPacket(new TutorialShowQuestionMark(QUESTION_MARK_ID, 1));
			}
		}
	}
}
