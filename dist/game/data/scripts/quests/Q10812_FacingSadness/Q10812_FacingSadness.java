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
package quests.Q10812_FacingSadness;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

import quests.Q10811_ExaltedOneWhoFacesTheLimit.Q10811_ExaltedOneWhoFacesTheLimit;

/**
 * Facing Sadness (10812)
 * @author Stayway, CostyKiller
 */
public class Q10812_FacingSadness extends Quest
{
	// Npc
	private static final int ELIKIA = 31620;
	
	// Items
	private static final int ELIKIA_CERTIFICATE = 45623;
	private static final int PROOF_OF_DISPOSAL = 45871;
	private static final ItemHolder LIONEL_HUNTERS_LIST_PART_1 = new ItemHolder(45627, 1);
	
	// Mobs
	private static final int[] MONSTERS =
	{
		// Hellbound Mobs
		23811, // Cantera Tanya
		23812, // Cantera Deathmoz
		23813, // Cantera Floxis
		23814, // Cantera Belika
		23815, // Cantera Bridget
		23354, // Decay Hannibal
		23355, // Armor Beast
		23356, // Klein Soldier
		23357, // Disorder Warrior
		23358, // Blow Archer
		23360, // Bizuard
		23361, // Mutated Fly
		23362, // Amos Soldier
		23363, // Amos Officer
		23364, // Amos Master
		23365, // Ailith Hunter
		23366, // Durable Charger
		23367, // Armor Beast
		23368, // Klein Soldier
		23369, // Disorder Warrior
		23370, // Blow Archer
		23372, // Bizuard
		23373, // Mutated Fly
		23384, // Smaug
		23385, // Lunatikan
		23386, // Jabberwok
		23387, // Kanzaroth
		23388, // Kandiloth
		23393, // Slaver
		23394, // Slaver
		23395, // Garion
		23396, // Garion Neti
		23397, // Desert Wendigo
		23398, // Koraza
		23399, // Bend Beetle
		19574, // Cowing
		// Raider's Crossroads Mobs
		23314, // Nerva Orc Raider
		23315, // Nerva Orc Archer
		23316, // Nerva Orc Priest
		23317, // Nerva Orc Wizard
		23318, // Nerva Orc Assassin
		23319, // Nerva Orc Ambusher
		23320, // Nerva Orc Merchant
		23321, // Nerva Orc Warrior
		23322, // Nerva Orc Prefect
		23323, // Nerva Orc Elite
		23324, // Nerva Kaiser
		29291, // Nerva Orc Raider
		29292, // Nerva Orc Elite
		29296, // Nerva Orc Assassin
		29297, // Nerva Orc Ambusher
	};
	
	// Misc
	private static final int MIN_LEVEL = 99;
	private static final int PROOF_OF_DISPOSAL_NEEDED = 8000;
	
	public Q10812_FacingSadness()
	{
		super(10812);
		addStartNpc(ELIKIA);
		addTalkId(ELIKIA);
		addKillId(MONSTERS);
		addCondMinLevel(MIN_LEVEL, "31620-09.htm");
		addCondStartedQuest(Q10811_ExaltedOneWhoFacesTheLimit.class.getSimpleName(), "31620-06.htm");
		registerQuestItems(PROOF_OF_DISPOSAL);
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
			case "31620-02.htm":
			case "31620-03.htm":
			{
				htmltext = event;
				break;
			}
			case "31620-04.html":
			{
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_1))
				{
					qs.startQuest();
					htmltext = event;
				}
				break;
			}
			case "31620-08.html":
			{
				if (qs.isCond(2))
				{
					if ((player.getLevel() >= MIN_LEVEL))
					{
						takeItems(player, PROOF_OF_DISPOSAL, PROOF_OF_DISPOSAL_NEEDED);
						giveItems(player, ELIKIA_CERTIFICATE, 1);
						addExpAndSp(player, 0, 498204432);
						qs.exitQuest(false, true);
						
						final Quest mainQ = ScriptManager.getInstance().getScript(Q10811_ExaltedOneWhoFacesTheLimit.class.getSimpleName());
						if (mainQ != null)
						{
							mainQ.notifyEvent("SUBQUEST_FINISHED_NOTIFY", npc, player);
						}
						
						htmltext = event;
					}
					else
					{
						htmltext = getNoQuestLevelRewardMsg(player);
					}
					break;
				}
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
				if (hasItem(player, LIONEL_HUNTERS_LIST_PART_1))
				{
					htmltext = "31620-01.htm";
				}
				else
				{
					htmltext = "noItem.html";
				}
				break;
			}
			case State.STARTED:
			{
				if (qs.isCond(1))
				{
					htmltext = "31620-05.html";
				}
				else if (qs.isCond(2) && (getQuestItemsCount(player, PROOF_OF_DISPOSAL) >= PROOF_OF_DISPOSAL_NEEDED))
				{
					htmltext = "31620-07.html";
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
		executeForEachPlayer(player, npc, isSummon, true, false);
	}
	
	@Override
	public void actionForEachPlayer(Player player, Npc npc, boolean isSummon)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1) && player.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			if (getQuestItemsCount(player, PROOF_OF_DISPOSAL) < PROOF_OF_DISPOSAL_NEEDED)
			{
				giveItems(player, PROOF_OF_DISPOSAL, 1);
				playSound(player, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
			
			if (getQuestItemsCount(player, PROOF_OF_DISPOSAL) >= PROOF_OF_DISPOSAL_NEEDED)
			{
				qs.setCond(2, true);
			}
		}
	}
}
