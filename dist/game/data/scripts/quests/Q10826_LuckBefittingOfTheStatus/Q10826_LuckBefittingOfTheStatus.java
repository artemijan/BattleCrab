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
package quests.Q10826_LuckBefittingOfTheStatus;

import java.util.HashMap;
import java.util.Map;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10823_ExaltedOneWhoShattersTheLimit.Q10823_ExaltedOneWhoShattersTheLimit;

/**
 * Luck Befitting of the Status (10826)
 * @URL https://l2wiki.com/Luck_Befitting_of_the_Status
 * @author Mobius
 */
public class Q10826_LuckBefittingOfTheStatus extends Quest
{
	// NPC
	private static final int BLACKSMITH_OF_MAMMON = 31126;
	
	// Items
	private static final int LADY_KNIFE = 45645;
	private static final int MERLOT_CERTIFICATE = 46056;
	private static final int KURTIZ_CERTIFICATE = 46057;
	private static final int GUSTAV_CERTIFICATE = 45636;
	
	// Rewards
	private static final int MAMMON_CERTIFICATE = 45635;
	private static final int SPELLBOOK_FATE_OF_THE_EXALTED = 46036;
	private static final Map<String, Integer> WEAPON_REWARDS = new HashMap<>();
	static
	{
		WEAPON_REWARDS.put("reward_shaper", 17416);
		WEAPON_REWARDS.put("reward_cutter", 17417);
		WEAPON_REWARDS.put("reward_slasher", 17418);
		WEAPON_REWARDS.put("reward_avenger", 17419);
		WEAPON_REWARDS.put("reward_fighter", 17420);
		WEAPON_REWARDS.put("reward_stormer", 17421);
		WEAPON_REWARDS.put("reward_thrower", 17422);
		WEAPON_REWARDS.put("reward_shooter", 17423);
		WEAPON_REWARDS.put("reward_buster", 17424);
		WEAPON_REWARDS.put("reward_caster", 17425);
		WEAPON_REWARDS.put("reward_retributer", 17426);
		WEAPON_REWARDS.put("reward_dualsword", 17427);
		WEAPON_REWARDS.put("reward_dualdagger", 17428);
		WEAPON_REWARDS.put("reward_dualblunt", 17429);
	}
	
	// Misc
	private static final int MIN_LEVEL = 100;
	
	public Q10826_LuckBefittingOfTheStatus()
	{
		super(10826);
		addStartNpc(BLACKSMITH_OF_MAMMON);
		addTalkId(BLACKSMITH_OF_MAMMON);
		addCondMinLevel(MIN_LEVEL, "31126-02.html");
		addCondStartedQuest(Q10823_ExaltedOneWhoShattersTheLimit.class.getSimpleName(), "31126-03.html");
		registerQuestItems(LADY_KNIFE);
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
			case "31126-04.htm":
			case "31126-05.htm":
			{
				htmltext = event;
				break;
			}
			case "31126-06.html":
			{
				if (qs.isCreated())
				{
					qs.startQuest();
					giveItems(player, LADY_KNIFE, 1);
					htmltext = event;
				}
				break;
			}
			case "31126-08.html":
			{
				if (qs.isCond(1))
				{
					giveItems(player, LADY_KNIFE, 1);
					htmltext = event;
				}
				break;
			}
		}
		
		if (event.startsWith("reward_") && qs.isCond(1) && (getEnchantLevel(player, LADY_KNIFE) >= 7))
		{
			if ((player.getLevel() >= MIN_LEVEL))
			{
				if (hasQuestItems(player, KURTIZ_CERTIFICATE, MERLOT_CERTIFICATE, GUSTAV_CERTIFICATE))
				{
					htmltext = "31126-15.html";
				}
				else
				{
					htmltext = "31126-14.html";
				}
				
				giveItems(player, WEAPON_REWARDS.get(event), 1);
				giveItems(player, MAMMON_CERTIFICATE, 1);
				giveItems(player, SPELLBOOK_FATE_OF_THE_EXALTED, 1);
				qs.exitQuest(false, true);
			}
			else
			{
				htmltext = getNoQuestLevelRewardMsg(player);
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
				htmltext = "31126-01.htm";
				break;
			}
			case State.STARTED:
			{
				if (!hasQuestItems(player, LADY_KNIFE))
				{
					htmltext = "31126-07.html";
				}
				else
				{
					final int enchantLevel = getEnchantLevel(player, LADY_KNIFE);
					if (enchantLevel == 0)
					{
						htmltext = "31126-09.html";
					}
					else if (enchantLevel < 5)
					{
						htmltext = "31126-10.html";
					}
					else if (enchantLevel < 7)
					{
						htmltext = "31126-11.html";
					}
					else if (enchantLevel == 7)
					{
						htmltext = "31126-12.html";
					}
					else
					{
						htmltext = "31126-13.html";
					}
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
}
