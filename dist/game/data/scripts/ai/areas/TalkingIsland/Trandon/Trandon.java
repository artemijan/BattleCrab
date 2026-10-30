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
package ai.areas.TalkingIsland.Trandon;

import java.util.Set;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.holders.player.SubClassHolder;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerSubChange;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.variables.PlayerVariables;
import org.l2jmobius.gameserver.network.clientpackets.RequestAcquireSkill;

/**
 * Trandon AI.
 * @author malyelfik
 */
public class Trandon extends Script
{
	// NPC
	private static final int NPC_ID = 33490;
	
	// Items
	private static final int SUB_CERTIFICATE = 10280;
	private static final int DUAL_CERTIFICATE = 36078;
	
	// Misc @formatter:off
	private static final int[] SUB_SKILL_LEVELS = {65, 70, 75, 80};
	private static final int[] DUAL_SKILL_LEVELS = {85, 90, 95, 99, 101, 103, 105};
	// @formatter:on
	private static final String SUB_CERTIFICATE_COUNT_VAR = "SUB_CERTIFICATE_COUNT";
	private static final String DUAL_CERTIFICATE_COUNT_VAR = "DUAL_CERTIFICATE_COUNT";
	
	private Trandon()
	{
		addStartNpc(NPC_ID);
		addFirstTalkId(NPC_ID);
		addTalkId(NPC_ID);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String[] substrings = event.split(" ");
		if (substrings.length < 1)
		{
			return null;
		}
		
		String htmltext = substrings[0];
		switch (htmltext)
		{
			case "33490.html":
			case "33490-01.html":
			case "33490-02.html":
			case "33490-03.html":
			case "33490-04.html":
			case "33490-05.html":
			case "33490-06.html":
			case "33490-07.html":
			case "33490-08.html":
			case "33490-09.html":
			case "33490-10.html":
			case "33490-11.html":
			case "33490-19.html":
			{
				break;
			}
			case "33490-12.html":
			{
				if ((player.getRace() == Race.ERTHEIA) || hasAllSubCertifications(player))
				{
					htmltext = "33490-15.html";
				}
				else if (!player.isSubClassActive())
				{
					htmltext = "33490-13.html";
				}
				else if (!player.isInventoryUnder90(false) || (player.getWeightPenalty() >= 2))
				{
					htmltext = "33490-14.html";
				}
				break;
			}
			case "subCertify":
			{
				if ((substrings.length < 2) || !player.isSubClassActive())
				{
					return null;
				}
				
				final int index = Integer.parseInt(substrings[1]);
				if ((index < 0) || (index > 3))
				{
					return null;
				}
				
				final int level = SUB_SKILL_LEVELS[index];
				if (player.getLevel() < level)
				{
					htmltext = getHtm(player, "33490-16.html").replace("%level%", String.valueOf(level));
				}
				else if (player.getVariables().hasVariable(getSubSkillVariableName(player, level)))
				{
					htmltext = "33490-17.html";
				}
				else
				{
					htmltext = getHtm(player, "33490-18.html");
					htmltext = htmltext.replace("%level%", String.valueOf(level));
					htmltext = htmltext.replace("%index%", String.valueOf(index));
				}
				break;
			}
			case "giveSubCertify":
			{
				if ((substrings.length < 2) || !player.isSubClassActive() || (player.getRace() == Race.ERTHEIA))
				{
					return null;
				}
				
				final int index = Integer.parseInt(substrings[1]);
				if ((index < 0) || (index > 3))
				{
					return null;
				}
				
				final int level = SUB_SKILL_LEVELS[index];
				final PlayerVariables vars = player.getVariables();
				if ((player.getLevel() < level) || vars.hasVariable(getSubSkillVariableName(player, level)))
				{
					htmltext = null;
				}
				else
				{
					final int subId = player.getPlayerClass().getId();
					final int currentCount = player.getVariables().getInt(SUB_CERTIFICATE_COUNT_VAR + subId, 0);
					if (currentCount < SUB_SKILL_LEVELS.length)
					{
						player.getVariables().set(SUB_CERTIFICATE_COUNT_VAR + subId, currentCount + 1);
						vars.set(getSubSkillVariableName(player, level), true);
						giveItems(player, SUB_CERTIFICATE, 1);
					}
					
					htmltext = "33490-20.html";
				}
				break;
			}
			case "learnSubSkill":
			{
				if (player.getRace() == Race.ERTHEIA)
				{
					htmltext = "33490-15.html";
				}
				else if (player.isSubClassActive() || !ownsAtLeastOneItem(player, SUB_CERTIFICATE))
				{
					htmltext = "33490-21.html";
				}
				else
				{
					RequestAcquireSkill.showSubSkillList(player);
					htmltext = null;
				}
				break;
			}
			case "deleteSubSkill":
			{
				// TODO: Unknown html
				if (player.getRace() == Race.ERTHEIA)
				{
					htmltext = "33490-15.html";
				}
				else if (player.isSubClassActive())
				{
					htmltext = "33490-21.html";
				}
				else if (player.getAdena() < PlayerConfig.FEE_DELETE_SUBCLASS_SKILLS)
				{
					htmltext = "33490-22.html";
				}
				else if (!hasSubCertificate(player))
				{
					htmltext = "33490-23.html";
				}
				else
				{
					htmltext = null; // TODO: Unknown html
					takeItems(player, SUB_CERTIFICATE, -1);
					player.getWarehouse().destroyItemByItemId(ItemProcessType.QUEST, SUB_CERTIFICATE, -1, player, npc);
					takeItems(player, Inventory.ADENA_ID, PlayerConfig.FEE_DELETE_SUBCLASS_SKILLS);
					for (SubClassHolder subclass : player.getSubClasses().values())
					{
						player.getVariables().remove(SUB_CERTIFICATE_COUNT_VAR + subclass.getId());
					}
					
					final PlayerVariables vars = player.getVariables();
					for (int i = 1; i <= 3; i++)
					{
						for (int lv : SUB_SKILL_LEVELS)
						{
							vars.remove("SubSkill-" + i + "-" + lv);
						}
					}
					
					takeSkills(player, "SubSkillList");
				}
				break;
			}
			case "33490-26.html":
			{
				// TODO: What happens when you have all dual certificates?
				if (!player.isDualClassActive())
				{
					htmltext = "33490-24.html";
				}
				else if (!player.isInventoryUnder90(false) || (player.getWeightPenalty() >= 2))
				{
					htmltext = "33490-25.html";
				}
				break;
			}
			case "dualCertify":
			{
				if ((substrings.length < 2) || !player.isDualClassActive())
				{
					return null;
				}
				
				final int index = Integer.parseInt(substrings[1]);
				if ((index < 0) || (index > (DUAL_SKILL_LEVELS.length - 1)))
				{
					return null;
				}
				
				final int level = DUAL_SKILL_LEVELS[index];
				final PlayerVariables vars = player.getVariables();
				if (vars.hasVariable(getDualSkillVariableName(level)))
				{
					htmltext = getHtm(player, "33490-27.html");
				}
				else if ((player.getLevel() < level) || (player.getStat().getBaseLevel() < level))
				{
					htmltext = getHtm(player, "33490-28.html");
				}
				else
				{
					final int currentCount = player.getVariables().getInt(DUAL_CERTIFICATE_COUNT_VAR, 0);
					if (currentCount < DUAL_SKILL_LEVELS.length)
					{
						player.getVariables().set(DUAL_CERTIFICATE_COUNT_VAR, currentCount + 1);
						vars.set(getDualSkillVariableName(level), true);
						giveItems(player, DUAL_CERTIFICATE, 1);
					}
					
					htmltext = getHtm(player, "33490-29.html");
				}
				
				htmltext = htmltext.replace("%level%", String.valueOf(level));
				break;
			}
			case "learnDualSkill":
			{
				// TODO: What happens when you have all dual-certificates used?
				if (player.isSubClassActive())
				{
					htmltext = "33490-30.html";
				}
				else if (!ownsAtLeastOneItem(player, DUAL_CERTIFICATE))
				{
					htmltext = "33490-31.html";
				}
				else if ((player.getLevel() < DUAL_SKILL_LEVELS[0]) || (player.getStat().getBaseLevel() < DUAL_SKILL_LEVELS[0]))
				{
					// This case should not happen
					htmltext = null;
				}
				else
				{
					RequestAcquireSkill.showDualSkillList(player);
					htmltext = null;
				}
				break;
			}
			case "deleteDualSkill":
			{
				if (player.isSubClassActive())
				{
					htmltext = "33490-30.html";
				}
				else if (player.getAdena() < PlayerConfig.FEE_DELETE_DUALCLASS_SKILLS)
				{
					htmltext = "33490-32.html";
				}
				else if (!hasDualCertificate(player))
				{
					htmltext = "33490-33.html";
				}
				else
				{
					htmltext = null; // TODO: Unknown html
					takeItems(player, DUAL_CERTIFICATE, -1);
					player.getWarehouse().destroyItemByItemId(ItemProcessType.QUEST, DUAL_CERTIFICATE, -1, player, npc);
					takeItems(player, Inventory.ADENA_ID, PlayerConfig.FEE_DELETE_DUALCLASS_SKILLS);
					player.getVariables().remove(DUAL_CERTIFICATE_COUNT_VAR);
					
					final PlayerVariables vars = player.getVariables();
					for (int lv : DUAL_SKILL_LEVELS)
					{
						vars.remove(getDualSkillVariableName(lv));
					}
					
					takeSkills(player, "DualSkillList");
				}
				break;
			}
			default:
			{
				htmltext = null;
			}
		}
		
		return htmltext;
	}
	
	// TODO: Move this to char skills
	@RegisterEvent(EventType.ON_PLAYER_SUB_CHANGE)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onSubChange(OnPlayerSubChange event)
	{
		final Player player = event.getPlayer();
		if (player.isDualClassActive() || !player.isSubClassActive())
		{
			giveSkills(player, "DualSkillList");
		}
		
		giveSkills(player, "SubSkillList");
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		if (player.isDualClassActive() || !player.isSubClassActive())
		{
			giveSkills(player, "DualSkillList");
		}
		
		giveSkills(player, "SubSkillList");
	}
	
	/**
	 * Checks if player has all sub certifications for current subclass
	 * @param player
	 * @return
	 */
	private final boolean hasAllSubCertifications(Player player)
	{
		if (!player.isSubClassActive())
		{
			return false;
		}
		
		final PlayerVariables vars = player.getVariables();
		for (int lv : SUB_SKILL_LEVELS)
		{
			if (!vars.hasVariable(getSubSkillVariableName(player, lv)))
			{
				return false;
			}
		}
		
		return true;
	}
	
	/**
	 * Checks if player has any sub certification
	 * @param player
	 * @return
	 */
	private final boolean hasSubCertificate(Player player)
	{
		final PlayerVariables vars = player.getVariables();
		final Set<Integer> subs = player.getSubClasses().keySet();
		for (int index : subs)
		{
			for (int lv : SUB_SKILL_LEVELS)
			{
				if (vars.hasVariable("SubSkill-" + index + "-" + lv))
				{
					return true;
				}
			}
		}
		
		return false;
	}
	
	/**
	 * Checks if player has any dual certification
	 * @param player
	 * @return
	 */
	private final boolean hasDualCertificate(Player player)
	{
		final PlayerVariables vars = player.getVariables();
		for (int lv : DUAL_SKILL_LEVELS)
		{
			if (vars.hasVariable(getDualSkillVariableName(lv)))
			{
				return true;
			}
		}
		
		return false;
	}
	
	private final String getSubSkillVariableName(Player player, int level)
	{
		return "SubSkill-" + player.getClassIndex() + "-" + level;
	}
	
	private final String getDualSkillVariableName(int level)
	{
		return "DualSkill-" + level;
	}
	
	private void takeSkills(Player player, String type)
	{
		final PlayerVariables vars = player.getVariables();
		final String list = vars.getString(type, "");
		if (!list.isEmpty())
		{
			final String[] skills = list.split(";");
			for (String skill : skills)
			{
				final String[] str = skill.split("-");
				final Skill sk = SkillData.getInstance().getSkill(Integer.parseInt(str[0]), Integer.parseInt(str[1]));
				player.removeSkill(sk);
			}
			
			vars.remove(type);
			player.sendSkillList();
		}
	}
	
	private void giveSkills(Player player, String type)
	{
		final String list = player.getVariables().getString(type, "");
		if (!list.isEmpty())
		{
			final String[] skills = list.split(";");
			for (String skill : skills)
			{
				final String[] str = skill.split("-");
				final Skill sk = SkillData.getInstance().getSkill(Integer.parseInt(str[0]), Integer.parseInt(str[1]));
				player.addSkill(sk, false);
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	public static void main(String[] args)
	{
		new Trandon();
	}
}
