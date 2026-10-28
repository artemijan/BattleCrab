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
package ai.areas.TalkingIsland.Hardin;

import java.util.List;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.data.xml.ClassListData;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.data.xml.SkillTreeData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.actor.enums.player.SubclassInfoType;
import org.l2jmobius.gameserver.model.olympiad.Olympiad;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillLearn;
import org.l2jmobius.gameserver.network.serverpackets.ExSubjobInfo;
import org.l2jmobius.gameserver.network.serverpackets.ExUserInfoInvenWeight;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

import quests.Q10472_WindsOfFateEncroachingShadows.Q10472_WindsOfFateEncroachingShadows;

/**
 * Hardin (Agent of Chaos) AI.
 * @author Mobius
 */
public class Hardin extends Script
{
	// NPC
	private static final int HARDIN = 33870;
	
	// Items
	private static final int CHAOS_ESSENCE = 36949;
	private static final int CHAOS_ESSENCE_DUAL_CLASS = 37494;
	private static final int CHAOS_POMANDER = 37374;
	private static final int CHAOS_POMANDER_DUAL_CLASS = 37375;
	
	private Hardin()
	{
		addStartNpc(HARDIN);
		addFirstTalkId(HARDIN);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = getHtmlMessage(player);
		if (htmltext != null)
		{
			return htmltext;
		}
		
		if (event.equals("list"))
		{
			final StringBuilder classes = new StringBuilder();
			final PlayerClass playerBaseTemplate = player.getBaseTemplate().getPlayerClass();
			for (PlayerClass c : PlayerClass.values())
			{
				if ((((c.level() != 4) && (c.getRace() != Race.ERTHEIA)) //
					|| (PlayerConfig.HARDIN_ENABLE_ERTHEIAS && (c.getRace() == Race.ERTHEIA) && (c.level() != 3))) //
					|| (!PlayerConfig.HARDIN_ENABLE_ERTHEIAS && (c.getRace() == Race.ERTHEIA)) //
					|| (c == player.getPlayerClass()) //
					|| (c == playerBaseTemplate))
				{
					continue;
				}
				
				if (!player.isDualClassActive() || (player.isDualClassActive() && PlayerConfig.HARDIN_ENABLE_DUALCLASS_CHECKS))
				{
					if (!PlayerConfig.HARDIN_ENABLE_ALL_RACES && (c.getRace() != player.getPlayerClass().getRace()))
					{
						continue;
					}
					
					if (!PlayerConfig.HARDIN_ENABLE_ALL_SPECS && (c.isMage() != player.isMageClass()))
					{
						continue;
					}
					
					if (PlayerConfig.HARDIN_SAME_AWAKEN_GROUP)
					{
						final String original = c.toString().contains("_") ? c.toString().substring(0, c.toString().indexOf('_') - 1) : c.toString();
						final String search = player.getPlayerClass().toString().contains("_") ? player.getPlayerClass().toString().substring(0, player.getPlayerClass().toString().indexOf('_') - 1) : player.getPlayerClass().toString();
						if (!original.equals(search))
						{
							continue;
						}
					}
					
					if (PlayerConfig.HARDIN_RETAIL_LIMITATIONS)
					{
						if ((c == PlayerClass.TYRR_MAESTRO) && (player.getRace() != Race.DWARF))
						{
							continue;
						}
						
						if ((c == PlayerClass.ISS_DOMINATOR) && (player.getRace() != Race.ORC))
						{
							continue;
						}
					}
				}
				
				classes.append("<button value=\"");
				classes.append(ClassListData.getInstance().getClass(c.getId()).getClassName());
				classes.append("\" action=\"bypass -h Script Hardin try_");
				classes.append(String.valueOf(c.getId()));
				classes.append("\" width=\"200\" height=\"31\" back=\"L2UI_CT1.HtmlWnd_DF_Awake_Down\" fore=\"L2UI_CT1.HtmlWnd_DF_Awake\"><br1>");
			}
			classes.append("<br><br><br><br><br>"); // prettify
			final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId());
			html.setHtml(getHtm(player, "33870-reawake_list.html"));
			html.replace("%CLASS_LIST%", classes.toString());
			player.sendPacket(html);
		}
		else if (event.contains("try"))
		{
			// Take item
			takeItems(player, player.isDualClassActive() ? CHAOS_ESSENCE_DUAL_CLASS : CHAOS_ESSENCE, 1);
			
			// Give item
			giveItems(player, player.isDualClassActive() ? CHAOS_POMANDER_DUAL_CLASS : CHAOS_POMANDER, 2);
			
			// Save original ClassId
			if (!player.isDualClassActive() && (player.getOriginalClass() == null))
			{
				player.setOriginalClass(player.getPlayerClass());
			}
			
			// Ertheias can only be female
			final PlayerClass newClass = PlayerClass.getPlayerClass(Integer.parseInt(event.replace("try_", "")));
			if ((newClass.getRace() == Race.ERTHEIA) && (player.getPlayerClass().getRace() != Race.ERTHEIA) && !player.getAppearance().isFemale())
			{
				player.getAppearance().setFemale();
			}
			
			// Change class
			player.setPlayerClass(newClass.getId());
			if (player.isDualClassActive())
			{
				player.getSubClasses().get(player.getClassIndex()).setPlayerClass(player.getActiveClass());
			}
			else
			{
				player.setBaseClass(player.getActiveClass());
			}
			
			// Adjustments
			SkillTreeData.getInstance().cleanSkillUponAwakening(player);
			for (SkillLearn skill : SkillTreeData.getInstance().getRaceSkillTree(player.getRace()))
			{
				player.addSkill(SkillData.getInstance().getSkill(skill.getSkillId(), skill.getSkillLevel()), true);
			}
			
			final List<Integer> removedSkillIds = PlayerConfig.HARDIN_REMOVED_SKILLS.get(newClass.getId());
			if (removedSkillIds != null)
			{
				for (int skillId : removedSkillIds)
				{
					final Skill skill = player.getKnownSkill(skillId);
					if (skill != null)
					{
						player.removeSkill(skill);
					}
				}
			}
			
			player.restoreDualSkills();
			player.store(false);
			player.broadcastUserInfo();
			player.sendSkillList();
			player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.CLASS_CHANGED));
			player.sendPacket(new ExUserInfoInvenWeight(player));
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final String htmltext = getHtmlMessage(player);
		return htmltext == null ? "33870-01.html" : htmltext;
	}
	
	private String getHtmlMessage(Player player)
	{
		if (player.getRace() == Race.ERTHEIA)
		{
			final QuestState qs = player.getQuestState(Q10472_WindsOfFateEncroachingShadows.class.getSimpleName());
			if ((qs != null) && (qs.getCond() >= 7) && (qs.getCond() <= 17))
			{
				return "33870-03.html";
			}
			
			if (!PlayerConfig.HARDIN_ENABLE_ERTHEIAS)
			{
				return "33870-02.html";
			}
		}
		
		if (!player.isInCategory(CategoryType.SIXTH_CLASS_GROUP))
		{
			return "33870-no.html";
		}
		
		if (player.isDualClassActive()) // dual class
		{
			if (!ownsAtLeastOneItem(player, CHAOS_ESSENCE_DUAL_CLASS))
			{
				return "33870-no_already_reawakened.html";
			}
		}
		else if (player.isSubClassActive()) // subclass
		{
			return "33870-no.html";
		}
		else if (!ownsAtLeastOneItem(player, CHAOS_ESSENCE)) // main class
		{
			return "33870-no_already_reawakened.html";
		}
		
		if (player.hasSummon())
		{
			return "33870-no_summon.html";
		}
		
		if (player.isInOlympiadMode() || (Olympiad.getInstance().getCompetitionDone(player.getObjectId()) > 0))
		{
			return "33870-no_olympiad.html";
		}
		
		return null;
	}
	
	public static void main(String[] args)
	{
		new Hardin();
	}
}
