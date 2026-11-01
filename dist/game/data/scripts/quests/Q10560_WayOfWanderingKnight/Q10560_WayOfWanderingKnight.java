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
package quests.Q10560_WayOfWanderingKnight;

import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.data.xml.CategoryData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.script.Faction;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowHtml;

// import ai.others.AdventurersGuide.AdventurersGuide;

/**
 * Way Of Wandering Knight (10560)
 * @URL https://l2wiki.com/Way_of_Wandering_Knight
 * @author NightBR
 */
public class Q10560_WayOfWanderingKnight extends Quest
{
	// NPCs
	private static final int HERPHAH = 34362;
	private static final int PENNY = 34413;
	private static final int ADVENTURERS_GUIDE = 32327;
	
	// Skills
	private static final SkillHolder KNIGHT = new SkillHolder(15648, 1); // Knight's Harmony (Adventurer)
	private static final SkillHolder WARRIOR = new SkillHolder(15649, 1); // Warrior's Harmony (Adventurer)
	private static final SkillHolder WIZARD = new SkillHolder(15650, 1); // Wizard's Harmony (Adventurer)
	private static final SkillHolder[] GROUP_BUFFS =
	{
		new SkillHolder(15642, 1), // Horn Melody (Adventurer)
		new SkillHolder(15643, 1), // Drum Melody (Adventurer)
		new SkillHolder(15644, 1), // Pipe Organ Melody (Adventurer)
		new SkillHolder(15645, 1), // Guitar Melody (Adventurer)
		new SkillHolder(15651, 1), // Prevailing Sonata (Adventurer)
		new SkillHolder(15652, 1), // Daring Sonata (Adventurer)
		new SkillHolder(15653, 1), // Refreshing Sonata (Adventurer)
	};
	
	// Reward's
	private static final long EXP = 1889719478;
	private static final int SP = 1700747;
	private static final int SOUL_SHOT_GRADE_R = 22433;
	private static final int BS_SHOT_GRADE_R = 22434;
	private static final int PA_ART_OF_SEDUCTION = 37928;
	private static final int ELEMENTARY_SEED_BRACELET = 48072;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	private static final int MAX_LEVEL = 97;
	
	public Q10560_WayOfWanderingKnight()
	{
		super(10560);
		addStartNpc(HERPHAH);
		addTalkId(HERPHAH, PENNY, ADVENTURERS_GUIDE);
		addCondMinLevel(MIN_LEVEL, "noLevel.html");
		addCondMaxLevel(MAX_LEVEL, "noLevel.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		final PlayerClass playerClass = player.getPlayerClass();
		
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "34362-02.htm":
			case "34362-03.htm":
			case "34362-11.html":
			case "32327-02.html":
			{
				htmltext = event;
				break;
			}
			case "34362-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "34362-05.html":
			{
				// Show Service/Help/Join a Clan page
				player.sendPacket(new ExTutorialShowId(46));
				htmltext = event;
				break;
			}
			case "34362-06.html":
			{
				// Show Service/Help/Creating a Clan page
				player.sendPacket(new ExTutorialShowId(22));
				htmltext = event;
				break;
			}
			case "34362-07.html":
			{
				qs.setCond(2, true);
				htmltext = event;
				break;
			}
			case "34362-10.html":
			{
				// Show Service/Help/Faction System page
				player.sendPacket(new TutorialShowHtml(npc.getObjectId(), "..\\L2Text\\help_faction.htm", TutorialShowHtml.LARGE_WINDOW));
				
				// TODO: Find the correct Id for player.sendPacket(new ExTutorialShowId(22));
				qs.setCond(4, true);
				htmltext = event;
				break;
			}
			case "34362-12.html":
			{
				qs.setCond(5, true);
				htmltext = event;
				break;
			}
			case "34362-14.html":
			{
				if (player.getFactionLevel(Faction.ADVENTURE_GUILD) >= 1)
				{
					giveItems(player, SOUL_SHOT_GRADE_R, 1500);
					giveItems(player, BS_SHOT_GRADE_R, 1500);
					giveItems(player, PA_ART_OF_SEDUCTION, 5);
					giveItems(player, ELEMENTARY_SEED_BRACELET, 1);
					addExpAndSp(player, EXP, SP);
					qs.exitQuest(QuestType.ONE_TIME, true);
					htmltext = event;
				}
				else
				{
					htmltext = getHtm(player, "noAmity.html").replace("%name%", "Herphah");
				}
				break;
			}
			case "32327-03.html":
			{
				if (CategoryData.getInstance().isInCategory(CategoryType.MAGE_GROUP, playerClass.getId()))
				{
					htmltext = getHtm(player, "32327-03.html").replace("%classbuff%", "Wizard");
				}
				else if (CategoryData.getInstance().isInCategory(CategoryType.ATTACKER_GROUP, playerClass.getId()))
				{
					htmltext = getHtm(player, "32327-03.html").replace("%classbuff%", "Warrior");
				}
				else if (CategoryData.getInstance().isInCategory(CategoryType.TANKER_GROUP, playerClass.getId()))
				{
					htmltext = getHtm(player, "32327-03.html").replace("%classbuff%", "Knight");
				}
				
				// Show Service/Help/Adventurer's Guide page
				player.sendPacket(new ExTutorialShowId(25));
				break;
			}
			case "Wizard":
			{
				htmltext = applyBuffs(npc, player, WIZARD.getSkill());
				qs.setCond(3, true);
				break;
			}
			case "Warrior":
			{
				htmltext = applyBuffs(npc, player, WARRIOR.getSkill());
				qs.setCond(3, true);
				break;
			}
			case "Knight":
			{
				htmltext = applyBuffs(npc, player, KNIGHT.getSkill());
				qs.setCond(3, true);
				break;
			}
			case "34413-02.html":
			{
				qs.setCond(6, true);
				htmltext = event;
				break;
			}
			case "34413-04.html":
			{
				// TODO: we need to add reward % of amity points to factions in all faction quests
				// check if reached level 1 with Adventurer's Guild Faction
				if (player.getFactionLevel(Faction.ADVENTURE_GUILD) >= 1)
				{
					qs.setCond(7, true);
					htmltext = event;
				}
				else
				{
					htmltext = getHtm(player, "noAmity.html").replace("%name%", "Penny");
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
				if (npc.getId() == HERPHAH)
				{
					htmltext = "34362-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case HERPHAH:
					{
						if (qs.isCond(3))
						{
							htmltext = "34362-09.html";
						}
						else if (qs.isCond(6) || qs.isCond(7))
						{
							htmltext = "34362-13.html";
						}
						break;
					}
					case PENNY:
					{
						if (qs.isCond(5))
						{
							htmltext = "34413-01.html";
						}
						else if (qs.isCond(6))
						{
							htmltext = "34413-03.html";
						}
						break;
					}
					case ADVENTURERS_GUIDE:
					{
						if (qs.isCond(2))
						{
							htmltext = "32327-01.html";
						}
						break;
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
	
	public String applyBuffs(Npc npc, Player player, Skill skill)
	{
		for (SkillHolder holder : GROUP_BUFFS)
		{
			SkillCaster.triggerCast(npc, player, holder.getSkill());
		}
		
		SkillCaster.triggerCast(npc, player, skill);
		
		return null;
	}
}
