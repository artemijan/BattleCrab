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
package handlers.chat.commands.admin;

import java.sql.Connection;
import java.sql.PreparedStatement;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.StringJoiner;
import java.util.StringTokenizer;
import java.util.logging.Logger;

import org.l2jmobius.commons.database.DatabaseFactory;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.config.PvpConfig;
import org.l2jmobius.gameserver.config.custom.AutoPlayConfig;
import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.data.sql.CharInfoTable;
import org.l2jmobius.gameserver.data.xml.ClassListData;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.data.xml.SkillTreeData;
import org.l2jmobius.gameserver.handler.IAdminCommandHandler;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Playable;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.Summon;
import org.l2jmobius.gameserver.model.actor.appearance.PlayerAppearance;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.actor.enums.player.SubclassInfoType;
import org.l2jmobius.gameserver.model.actor.instance.Pet;
import org.l2jmobius.gameserver.model.actor.stat.PlayerStat;
import org.l2jmobius.gameserver.model.clan.Clan;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.groups.PartyDistributionType;
import org.l2jmobius.gameserver.model.html.PageBuilder;
import org.l2jmobius.gameserver.model.html.PageResult;
import org.l2jmobius.gameserver.model.html.formatters.BypassParserFormatter;
import org.l2jmobius.gameserver.model.html.pagehandlers.NextPrevPageHandler;
import org.l2jmobius.gameserver.model.html.styles.ButtonsStyle;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillLearn;
import org.l2jmobius.gameserver.model.stats.Stat;
import org.l2jmobius.gameserver.network.GameClient;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.ExSubjobInfo;
import org.l2jmobius.gameserver.network.serverpackets.ExUserInfoInvenWeight;
import org.l2jmobius.gameserver.network.serverpackets.GMViewItemList;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;
import org.l2jmobius.gameserver.network.serverpackets.PartySmallWindowAll;
import org.l2jmobius.gameserver.network.serverpackets.PartySmallWindowDeleteAll;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

/**
 * EditChar admin command implementation.
 */
public class AdminEditChar implements IAdminCommandHandler
{
	private static final Logger LOGGER = Logger.getLogger(AdminEditChar.class.getName());
	
	private static final String[] ADMIN_COMMANDS =
	{
		"admin_edit_character",
		"admin_current_player",
		"admin_setreputation", // sets reputation of target char to any amount. //setreputation <amout>
		"admin_nokarma", // sets reputation to 0 if it is negative.
		"admin_setfame", // sets fame of target char to any amount. //setfame <fame>
		"admin_character_list", // same as character_info, kept for compatibility purposes
		"admin_character_info", // given a player name, displays an information window
		"admin_show_characters", // list of characters
		"admin_find_character", // find a player by his name or a part of it (case-insensitive)
		"admin_find_ip", // find all the player connections from a given IPv4 number
		"admin_find_account", // list all the characters from an account (useful for GMs w/o DB access)
		"admin_find_dualbox", // list all the IPs with more than 1 char logged in (dualbox)
		"admin_strict_find_dualbox",
		"admin_tracert",
		"admin_rec", // gives recommendation points
		"admin_settitle", // changes char title
		"admin_changename", // changes char name
		"admin_setsex", // changes characters' sex
		"admin_setcolor", // change charnames' color display
		"admin_settcolor", // change char title color
		"admin_setclass", // changes chars' classId
		"admin_setpk", // changes PK count
		"admin_setpvp", // changes PVP count
		"admin_set_pvp_flag",
		"admin_fullfood", // fulfills a pet's food bar
		"admin_remove_clan_penalty", // removes clan penalties
		"admin_summon_info", // displays an information window about target summon
		"admin_unsummon",
		"admin_summon_setlvl",
		"admin_show_pet_inv",
		"admin_partyinfo",
		"admin_setnoble",
		"admin_set_hp",
		"admin_set_mp",
		"admin_set_cp",
		"admin_setparam",
		"admin_unsetparam"
	};
	
	@Override
	public boolean onCommand(String command, Player activeChar)
	{
		if (command.equals("admin_current_player"))
		{
			showCharacterInfo(activeChar, activeChar);
		}
		else if (command.startsWith("admin_character_info"))
		{
			final String[] data = command.split(" ");
			if ((data.length > 1))
			{
				showCharacterInfo(activeChar, World.getInstance().getPlayer(data[1]));
			}
			else if ((activeChar.getTarget() != null) && activeChar.getTarget().isPlayer())
			{
				showCharacterInfo(activeChar, activeChar.getTarget().asPlayer());
			}
			else
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
			}
		}
		else if (command.startsWith("admin_character_list"))
		{
			listCharacters(activeChar, 0);
		}
		else if (command.startsWith("admin_show_characters"))
		{
			try
			{
				final String val = command.substring(22).replace("page=", "");
				final int page = Integer.parseInt(val);
				listCharacters(activeChar, page);
			}
			catch (StringIndexOutOfBoundsException e)
			{
				// Case of empty page number
				activeChar.sendSysMessage("Usage: //show_characters <page_number>");
			}
		}
		else if (command.startsWith("admin_find_character"))
		{
			try
			{
				final String val = command.substring(21);
				findCharacter(activeChar, val);
			}
			catch (StringIndexOutOfBoundsException e)
			{ // Case of empty character name
				activeChar.sendSysMessage("Usage: //find_character <character_name>");
				listCharacters(activeChar, 0);
			}
		}
		else if (command.startsWith("admin_find_ip"))
		{
			try
			{
				final String val = command.substring(14);
				findCharactersPerIp(activeChar, val);
			}
			catch (Exception e)
			{ // Case of empty or malformed IP number
				activeChar.sendSysMessage("Usage: //find_ip <www.xxx.yyy.zzz>");
				listCharacters(activeChar, 0);
			}
		}
		else if (command.startsWith("admin_find_account"))
		{
			try
			{
				final String val = command.substring(19);
				findCharactersPerAccount(activeChar, val);
			}
			catch (Exception e)
			{ // Case of empty or malformed player name
				activeChar.sendSysMessage("Usage: //find_account <player_name>");
				listCharacters(activeChar, 0);
			}
		}
		else if (command.startsWith("admin_edit_character"))
		{
			final String[] data = command.split(" ");
			if ((data.length > 1))
			{
				editCharacter(activeChar, data[1]);
			}
			else if ((activeChar.getTarget() != null) && activeChar.getTarget().isPlayer())
			{
				editCharacter(activeChar, null);
			}
			else
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
			}
		}
		else if (command.startsWith("admin_setreputation"))
		{
			try
			{
				final String val = command.substring(20);
				final int reputation = Integer.parseInt(val);
				setTargetReputation(activeChar, reputation);
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //setreputation <new_reputation_value>");
			}
		}
		else if (command.startsWith("admin_nokarma"))
		{
			if ((activeChar.getTarget() == null) || !activeChar.getTarget().isPlayer())
			{
				activeChar.sendSysMessage("You must target a player.");
				return false;
			}
			
			if (activeChar.getTarget().asPlayer().getReputation() < 0)
			{
				setTargetReputation(activeChar, 0);
			}
		}
		else if (command.startsWith("admin_setpk"))
		{
			try
			{
				final String val = command.substring(12);
				final int pk = Integer.parseInt(val);
				final WorldObject target = activeChar.getTarget();
				if (target.isPlayer())
				{
					final Player player = target.asPlayer();
					player.setPkKills(pk);
					player.broadcastUserInfo();
					player.sendMessage("A GM changed your PK count to " + pk);
					activeChar.sendMessage(player.getName() + "'s PK count changed to " + pk);
				}
				else
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				}
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //setpk <pk_count>");
			}
		}
		else if (command.startsWith("admin_setpvp"))
		{
			try
			{
				final String val = command.substring(13);
				final int pvp = Integer.parseInt(val);
				final WorldObject target = activeChar.getTarget();
				if ((target != null) && target.isPlayer())
				{
					final Player player = target.asPlayer();
					player.setPvpKills(pvp);
					player.updatePvpTitleAndColor(false);
					player.broadcastUserInfo();
					player.sendMessage("A GM changed your PVP count to " + pvp);
					activeChar.sendMessage(player.getName() + "'s PVP count changed to " + pvp);
				}
				else
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				}
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //setpvp <pvp_count>");
			}
		}
		else if (command.startsWith("admin_setfame"))
		{
			if (!PlayerConfig.FAME_SYSTEM_ENABLED)
			{
				activeChar.sendMessage("Fame system is not enabled on the server!");
				return false;
			}
			
			try
			{
				final String val = command.substring(14);
				final int fame = Integer.parseInt(val);
				final WorldObject target = activeChar.getTarget();
				if ((target != null) && target.isPlayer())
				{
					final Player player = target.asPlayer();
					player.setFame(fame);
					player.broadcastUserInfo();
					player.sendMessage("A GM changed your Reputation points to " + fame);
					activeChar.sendMessage(player.getName() + "'s Fame changed to " + fame);
				}
				else
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				}
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //setfame <new_fame_value>");
			}
		}
		else if (command.startsWith("admin_rec"))
		{
			try
			{
				final String val = command.substring(10);
				final int recVal = Integer.parseInt(val);
				final WorldObject target = activeChar.getTarget();
				if ((target != null) && target.isPlayer())
				{
					final Player player = target.asPlayer();
					player.setRecomHave(recVal);
					player.broadcastUserInfo();
					player.sendMessage("A GM changed your Recommend points to " + recVal);
					activeChar.sendMessage(player.getName() + "'s Recommend changed to " + recVal);
				}
				else
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				}
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //rec number");
			}
		}
		else if (command.startsWith("admin_setclass"))
		{
			try
			{
				final String val = command.substring(15).trim();
				final int classidval = Integer.parseInt(val);
				final WorldObject target = activeChar.getTarget();
				if ((target == null) || !target.isPlayer())
				{
					return false;
				}
				
				final Player player = target.asPlayer();
				if ((PlayerClass.getPlayerClass(classidval) != null) && (player.getPlayerClass().getId() != classidval))
				{
					if (!player.isDualClassActive() && !player.isSubClassActive())
					{
						player.resetOriginalClass(); // revert agent of chaos changes
					}
					
					player.setPlayerClass(classidval);
					if (player.isSubClassActive())
					{
						player.getSubClasses().get(player.getClassIndex()).setPlayerClass(player.getActiveClass());
					}
					else
					{
						player.setBaseClass(player.getActiveClass());
					}
					
					// Sex checks.
					final PlayerAppearance appearance = player.getAppearance();
					if (player.getRace() == Race.KAMAEL)
					{
						switch (classidval)
						{
							case 123: // Soldier (Male)
							case 125: // Trooper
							case 127: // Berserker
							case 128: // Soul Breaker (Male)
							case 131: // Doombringer
							case 132: // Soul Hound (Male)
							case 157: // Tyrr Doombringer
							{
								if (appearance.isFemale())
								{
									appearance.setMale();
								}
								break;
							}
							case 124: // Soldier (Female)
							case 126: // Warder
							case 129: // Soul Breaker (Female)
							case 130: // Arbalester
							case 133: // Soul Hound (Female)
							case 134: // Trickster
							case 165: // Yul Trickster
							{
								if (!appearance.isFemale())
								{
									appearance.setFemale();
								}
								break;
							}
						}
					}
					
					// Ertheia sex check.
					if (player.getRace() == Race.ERTHEIA)
					{
						appearance.setFemale();
					}
					
					final String newclass = ClassListData.getInstance().getClass(player.getPlayerClass()).getClassName();
					player.getEffectList().stopAllEffectsWithoutExclusions(true, true);
					if (player.isInCategory(CategoryType.SIXTH_CLASS_GROUP))
					{
						SkillTreeData.getInstance().cleanSkillUponAwakening(player);
						for (SkillLearn skill : SkillTreeData.getInstance().getRaceSkillTree(player.getRace()))
						{
							player.addSkill(SkillData.getInstance().getSkill(skill.getSkillId(), skill.getSkillLevel()), true);
						}
					}
					
					for (Skill oldSkill : player.getAllSkills())
					{
						player.removeSkill(oldSkill, true, true);
					}
					
					player.broadcastUserInfo();
					player.sendSkillList();
					player.rewardSkills();
					player.store(false);
					player.sendPacket(new ExSubjobInfo(player, SubclassInfoType.CLASS_CHANGED));
					player.sendPacket(new ExUserInfoInvenWeight(player));
					player.sendMessage("A GM changed your class to " + newclass + ".");
					activeChar.sendMessage(player.getName() + " is a " + newclass + ".");
				}
				else
				{
					activeChar.sendSysMessage("Usage: //setclass <valid_new_classid>");
				}
			}
			catch (StringIndexOutOfBoundsException e)
			{
				AdminHtml.showAdminHtml(activeChar, "setclass/human_fighter.htm");
			}
			catch (NumberFormatException e)
			{
				activeChar.sendSysMessage("Usage: //setclass <valid_new_classid>");
			}
		}
		else if (command.startsWith("admin_settitle"))
		{
			try
			{
				final String val = command.substring(15);
				final WorldObject target = activeChar.getTarget();
				Player player = null;
				if ((target != null) && target.isPlayer())
				{
					player = target.asPlayer();
				}
				else
				{
					return false;
				}
				
				player.setTitle(val);
				player.sendMessage("Your title has been changed by a GM");
				player.broadcastTitleInfo();
			}
			catch (StringIndexOutOfBoundsException e)
			{ // Case of empty character title
				activeChar.sendSysMessage("You need to specify the new title.");
			}
		}
		else if (command.startsWith("admin_changename"))
		{
			try
			{
				final String val = command.substring(17);
				final WorldObject target = activeChar.getTarget();
				Player player = null;
				if ((target != null) && target.isPlayer())
				{
					player = target.asPlayer();
				}
				else
				{
					return false;
				}
				
				if (CharInfoTable.getInstance().doesCharNameExist(val))
				{
					activeChar.sendSysMessage("Warning, player " + val + " already exists");
					return false;
				}
				
				player.setName(val);
				CharInfoTable.getInstance().addName(player);
				player.storeMe();
				
				activeChar.sendSysMessage("Changed name to " + val);
				player.sendMessage("Your name has been changed by a GM.");
				player.broadcastUserInfo();
				
				final Party party = player.getParty();
				if (party != null)
				{
					// Delete party window for other party members
					party.broadcastToPartyMembers(player, PartySmallWindowDeleteAll.STATIC_PACKET);
					for (Player member : party.getMembers())
					{
						// And re-add
						if (member != player)
						{
							member.sendPacket(new PartySmallWindowAll(member, party));
						}
					}
				}
				
				final Clan clan = player.getClan();
				if (clan != null)
				{
					clan.broadcastClanStatus();
				}
			}
			catch (StringIndexOutOfBoundsException e)
			{ // Case of empty character name
				activeChar.sendSysMessage("Usage: //setname new_name_for_target");
			}
		}
		else if (command.startsWith("admin_setsex"))
		{
			final WorldObject target = activeChar.getTarget();
			Player player = null;
			if ((target != null) && target.isPlayer())
			{
				player = target.asPlayer();
			}
			else
			{
				return false;
			}
			
			final PlayerAppearance appearance = player.getAppearance();
			if (appearance.isFemale())
			{
				appearance.setMale();
			}
			else
			{
				appearance.setFemale();
			}
			
			player.sendMessage("Your gender has been changed by a GM");
			player.broadcastUserInfo();
		}
		else if (command.startsWith("admin_setcolor"))
		{
			try
			{
				final String val = command.substring(15);
				final WorldObject target = activeChar.getTarget();
				Player player = null;
				if ((target != null) && target.isPlayer())
				{
					player = target.asPlayer();
				}
				else
				{
					return false;
				}
				
				player.getAppearance().setNameColor(Integer.decode("0x" + val));
				player.sendMessage("Your name color has been changed by a GM");
				player.broadcastUserInfo();
			}
			catch (Exception e)
			{ // Case of empty color or invalid hex string
				activeChar.sendSysMessage("You need to specify a valid new color.");
			}
		}
		else if (command.startsWith("admin_settcolor"))
		{
			try
			{
				final String val = command.substring(16);
				final WorldObject target = activeChar.getTarget();
				Player player = null;
				if ((target != null) && target.isPlayer())
				{
					player = target.asPlayer();
				}
				else
				{
					return false;
				}
				
				player.getAppearance().setTitleColor(Integer.decode("0x" + val));
				player.sendMessage("Your title color has been changed by a GM");
				player.broadcastUserInfo();
			}
			catch (Exception e)
			{ // Case of empty color or invalid hex string
				activeChar.sendSysMessage("You need to specify a valid new color.");
			}
		}
		else if (command.startsWith("admin_fullfood"))
		{
			final WorldObject target = activeChar.getTarget();
			if ((target != null) && target.isPet())
			{
				final Pet targetPet = target.asPet();
				targetPet.setCurrentFed(targetPet.getMaxFed());
				targetPet.broadcastStatusUpdate();
			}
			else
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
			}
		}
		else if (command.startsWith("admin_remove_clan_penalty"))
		{
			try
			{
				final StringTokenizer st = new StringTokenizer(command, " ");
				if (st.countTokens() != 3)
				{
					activeChar.sendSysMessage("Usage: //remove_clan_penalty join|create charname");
					return false;
				}
				
				st.nextToken();
				
				final boolean changeCreateExpiryTime = st.nextToken().equalsIgnoreCase("create");
				final String playerName = st.nextToken();
				Player player = null;
				player = World.getInstance().getPlayer(playerName);
				if (player == null)
				{
					final Connection con = DatabaseFactory.getConnection();
					final PreparedStatement ps = con.prepareStatement("UPDATE characters SET " + (changeCreateExpiryTime ? "clan_create_expiry_time" : "clan_join_expiry_time") + " WHERE char_name=? LIMIT 1");
					ps.setString(1, playerName);
					ps.execute();
				}
				else if (changeCreateExpiryTime) // removing penalty
				{
					player.setClanCreateExpiryTime(0);
				}
				else
				{
					player.setClanJoinExpiryTime(0);
				}
				
				activeChar.sendSysMessage("Clan penalty successfully removed to character: " + playerName);
			}
			catch (Exception e)
			{
				LOGGER.warning(e.toString());
			}
		}
		else if (command.startsWith("admin_find_dualbox"))
		{
			int multibox = 2;
			try
			{
				final String val = command.substring(19);
				multibox = Integer.parseInt(val);
				if (multibox < 1)
				{
					activeChar.sendSysMessage("Usage: //find_dualbox [number > 0]");
					return false;
				}
			}
			catch (Exception e)
			{
			}
			
			findDualbox(activeChar, multibox);
		}
		else if (command.startsWith("admin_strict_find_dualbox"))
		{
			int multibox = 2;
			try
			{
				final String val = command.substring(26);
				multibox = Integer.parseInt(val);
				if (multibox < 1)
				{
					activeChar.sendSysMessage("Usage: //strict_find_dualbox [number > 0]");
					return false;
				}
			}
			catch (Exception e)
			{
			}
			
			findDualboxStrict(activeChar, multibox);
		}
		else if (command.startsWith("admin_tracert"))
		{
			final String[] data = command.split(" ");
			Player pl = null;
			if ((data.length > 1))
			{
				pl = World.getInstance().getPlayer(data[1]);
			}
			else
			{
				final WorldObject target = activeChar.getTarget();
				if ((target != null) && target.isPlayer())
				{
					pl = target.asPlayer();
				}
			}
			
			if (pl == null)
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				return false;
			}
			
			final GameClient client = pl.getClient();
			if (client == null)
			{
				activeChar.sendSysMessage("Client is null.");
				return false;
			}
			
			if (client.isDetached())
			{
				activeChar.sendSysMessage("Client is detached.");
				return false;
			}
			
			String ip;
			final int[][] trace = client.getTrace();
			for (int i = 0; i < trace.length; i++)
			{
				ip = "";
				for (int o = 0; o < trace[0].length; o++)
				{
					ip = ip + trace[i][o];
					if (o != (trace[0].length - 1))
					{
						ip = ip + ".";
					}
				}
				
				activeChar.sendSysMessage("Hop" + i + ": " + ip);
			}
		}
		else if (command.startsWith("admin_summon_info"))
		{
			final WorldObject target = activeChar.getTarget();
			if ((target != null) && target.isSummon())
			{
				gatherSummonInfo(target.asSummon(), activeChar);
			}
			else
			{
				activeChar.sendSysMessage("Invalid target.");
			}
		}
		else if (command.startsWith("admin_unsummon"))
		{
			final WorldObject target = activeChar.getTarget();
			if ((target != null) && target.isSummon())
			{
				target.asSummon().unSummon(target.asSummon().getOwner());
			}
			else
			{
				activeChar.sendSysMessage("Usable only with Pets/Summons");
			}
		}
		else if (command.startsWith("admin_summon_setlvl"))
		{
			final WorldObject target = activeChar.getTarget();
			if ((target != null) && target.isPet())
			{
				final Pet pet = target.asPet();
				try
				{
					final String val = command.substring(20);
					final int level = Integer.parseInt(val);
					final long oldexp = pet.getStat().getExp();
					final long newexp = pet.getStat().getExpForLevel(level);
					if (oldexp > newexp)
					{
						pet.getStat().removeExp(oldexp - newexp);
					}
					else if (oldexp < newexp)
					{
						pet.getStat().addExp(newexp - oldexp);
					}
				}
				catch (Exception e)
				{
				}
			}
			else
			{
				activeChar.sendSysMessage("Usable only with Pets");
			}
		}
		else if (command.startsWith("admin_show_pet_inv"))
		{
			WorldObject target;
			try
			{
				final String val = command.substring(19);
				final int objId = Integer.parseInt(val);
				target = World.getInstance().getPet(objId);
			}
			catch (Exception e)
			{
				target = activeChar.getTarget();
			}
			
			if ((target != null) && target.isPet())
			{
				activeChar.sendPacket(new GMViewItemList(1, target.asPet()));
			}
			else
			{
				activeChar.sendSysMessage("Usable only with Pets");
			}
		}
		else if (command.startsWith("admin_partyinfo"))
		{
			WorldObject target;
			try
			{
				final String val = command.substring(16);
				target = World.getInstance().getPlayer(val);
				if (target == null)
				{
					target = activeChar.getTarget();
				}
			}
			catch (Exception e)
			{
				target = activeChar.getTarget();
			}
			
			if (target.isPlayer())
			{
				if (target.asPlayer().isInParty())
				{
					gatherPartyInfo(target.asPlayer(), activeChar);
				}
				else
				{
					activeChar.sendSysMessage(target.getName() + " is not in a party.");
				}
			}
			else
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
			}
		}
		else if (command.equals("admin_setnoble"))
		{
			Player player = null;
			if ((activeChar.getTarget() != null) && (activeChar.getTarget().isPlayer()))
			{
				player = activeChar.getTarget().asPlayer();
			}
			else
			{
				player = activeChar;
			}
			
			if (player != null)
			{
				switch (player.getNobleLevel())
				{
					case 0:
					{
						player.setNobleLevel(1);
						player.sendMessage("A GM added you nobless status!");
						break;
					}
					case 1:
					{
						player.setNobleLevel(2);
						player.sendMessage("A GM changed your nobless status to exalted!");
						break;
					}
					case 2:
					{
						player.setNobleLevel(0);
						player.sendMessage("A GM removed your nobless status!");
						break;
					}
				}
				
				if (player.getObjectId() != activeChar.getObjectId())
				{
					activeChar.sendSysMessage("You've changed nobless status of: " + player.getName());
				}
				
				player.broadcastUserInfo();
			}
		}
		else if (command.startsWith("admin_set_hp"))
		{
			final String[] data = command.split(" ");
			try
			{
				final WorldObject target = activeChar.getTarget();
				if ((target == null) || !target.isCreature())
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
					return false;
				}
				
				target.asCreature().setCurrentHp(Double.parseDouble(data[1]));
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //set_hp 1000");
			}
		}
		else if (command.startsWith("admin_set_mp"))
		{
			final String[] data = command.split(" ");
			try
			{
				final WorldObject target = activeChar.getTarget();
				if ((target == null) || !target.isCreature())
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
					return false;
				}
				
				target.asCreature().setCurrentMp(Double.parseDouble(data[1]));
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //set_mp 1000");
			}
		}
		else if (command.startsWith("admin_set_cp"))
		{
			final String[] data = command.split(" ");
			try
			{
				final WorldObject target = activeChar.getTarget();
				if ((target == null) || !target.isCreature())
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
					return false;
				}
				
				target.asCreature().setCurrentCp(Double.parseDouble(data[1]));
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //set_cp 1000");
			}
		}
		else if (command.startsWith("admin_set_pvp_flag"))
		{
			try
			{
				final WorldObject target = activeChar.getTarget();
				if ((target == null) || !target.isPlayable())
				{
					activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
					return false;
				}
				
				final Playable playable = target.asPlayable();
				playable.updatePvPFlag(Math.abs(playable.getPvpFlag() - 1));
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Usage: //set_pvp_flag");
			}
		}
		else if (command.startsWith("admin_setparam"))
		{
			final WorldObject target = activeChar.getTarget();
			if ((target == null) || !target.isCreature())
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				return false;
			}
			
			final StringTokenizer st = new StringTokenizer(command, " ");
			st.nextToken(); // admin_setparam
			if (!st.hasMoreTokens())
			{
				activeChar.sendSysMessage("Syntax: //setparam <stat> <value>");
				return false;
			}
			
			final String statName = st.nextToken();
			if (!st.hasMoreTokens())
			{
				activeChar.sendSysMessage("Syntax: //setparam <stat> <value>");
				return false;
			}
			
			try
			{
				Stat stat = null;
				for (Stat stats : Stat.values())
				{
					if (statName.equalsIgnoreCase(stats.name()) || statName.equalsIgnoreCase(stats.getValue()))
					{
						stat = stats;
						break;
					}
				}
				
				if (stat == null)
				{
					activeChar.sendSysMessage("Couldn't find such stat!");
					return false;
				}
				
				final double value = Double.parseDouble(st.nextToken());
				final Creature targetCreature = target.asCreature();
				if (value >= 0)
				{
					targetCreature.getStat().addFixedValue(stat, value);
					targetCreature.getStat().recalculateStats(true);
					activeChar.sendSysMessage("Fixed stat: " + stat + " has been set to " + value);
				}
				else
				{
					activeChar.sendSysMessage("Non negative values are only allowed!");
				}
			}
			catch (Exception e)
			{
				activeChar.sendSysMessage("Syntax: //setparam <stat> <value>");
				return false;
			}
		}
		else if (command.startsWith("admin_unsetparam"))
		{
			final WorldObject target = activeChar.getTarget();
			if ((target == null) || !target.isCreature())
			{
				activeChar.sendPacket(SystemMessageId.INVALID_TARGET);
				return false;
			}
			
			final StringTokenizer st = new StringTokenizer(command, " ");
			st.nextToken(); // admin_setparam
			if (!st.hasMoreTokens())
			{
				activeChar.sendSysMessage("Syntax: //unsetparam <stat>");
				return false;
			}
			
			final String statName = st.nextToken();
			Stat stat = null;
			for (Stat stats : Stat.values())
			{
				if (statName.equalsIgnoreCase(stats.name()) || statName.equalsIgnoreCase(stats.getValue()))
				{
					stat = stats;
					break;
				}
			}
			
			if (stat == null)
			{
				activeChar.sendSysMessage("Couldn't find such stat!");
				return false;
			}
			
			final Creature targetCreature = target.asCreature();
			targetCreature.getStat().removeFixedValue(stat);
			targetCreature.getStat().recalculateStats(true);
			activeChar.sendSysMessage("Fixed stat: " + stat + " has been removed.");
		}
		
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return ADMIN_COMMANDS;
	}
	
	private void listCharacters(Player activeChar, int page)
	{
		final List<Player> players = new ArrayList<>(World.getInstance().getPlayers());
		players.sort(Comparator.comparingLong(Player::getUptime));
		
		final NpcHtmlMessage html = new NpcHtmlMessage(0, 1);
		html.setFile(activeChar, "data/html/admin/charlist.htm");
		
		final PageResult result = PageBuilder.newBuilder(players, 20, "bypass -h admin_show_characters").currentPage(page).pageHandler(NextPrevPageHandler.INSTANCE).formatter(BypassParserFormatter.INSTANCE).style(ButtonsStyle.INSTANCE).bodyHandler((pages, player, sb) ->
		{
			sb.append("<tr>");
			sb.append("<td width=80><a action=\"bypass -h admin_character_info " + player.getName() + "\">" + ((player.isInOfflineMode() ? ("<font color=\"808080\">" + player.getName() + "</font>") : player.getName()) + "</a></td>"));
			sb.append("<td width=110>" + ClassListData.getInstance().getClass(player.getPlayerClass()).getClassName() + "</td><td width=40>" + player.getLevel() + "</td>");
			sb.append("</tr>");
		}).build();
		
		if (result.getPages() > 1)
		{
			html.replace("%pages%", "<table width=280 cellspacing=0><tr>" + result.getPagerTemplate() + "</tr></table>");
		}
		else
		{
			html.replace("%pages%", "");
		}
		
		html.replace("%players%", result.getBodyTemplate().toString());
		activeChar.sendPacket(html);
	}
	
	private void showCharacterInfo(Player activeChar, Player targetPlayer)
	{
		Player player = targetPlayer;
		if (player == null)
		{
			final WorldObject target = activeChar.getTarget();
			if ((target != null) && target.isPlayer())
			{
				player = target.asPlayer();
			}
			else
			{
				return;
			}
		}
		else
		{
			activeChar.setTarget(player);
		}
		
		gatherCharacterInfo(activeChar, player, "charinfo.htm");
	}
	
	/**
	 * Retrieve and replace player's info in filename htm file, sends it to activeChar as NpcHtmlMessage.
	 * @param activeChar
	 * @param player
	 * @param filename
	 */
	private void gatherCharacterInfo(Player activeChar, Player player, String filename)
	{
		String ip = "N/A";
		if (player == null)
		{
			activeChar.sendSysMessage("Player is null.");
			return;
		}
		
		final GameClient client = player.getClient();
		if (client == null)
		{
			activeChar.sendSysMessage("Client is null.");
		}
		else if (client.isDetached())
		{
			activeChar.sendSysMessage("Client is detached.");
		}
		else
		{
			ip = client.getIp();
		}
		
		final PlayerStat stat = player.getStat();
		final NpcHtmlMessage adminReply = new NpcHtmlMessage(0, 1);
		adminReply.setFile(activeChar, "data/html/admin/" + filename);
		adminReply.replace("%name%", player.getName());
		adminReply.replace("%level%", String.valueOf(player.getLevel()));
		adminReply.replace("%clan%", String.valueOf(player.getClan() != null ? "<a action=\"bypass -h admin_clan_info " + player.getObjectId() + "\">" + player.getClan().getName() + "</a>" : null));
		adminReply.replace("%xp%", String.valueOf(player.getExp()));
		adminReply.replace("%sp%", String.valueOf(player.getSp()));
		adminReply.replace("%class%", ClassListData.getInstance().getClass(player.getPlayerClass()).getClassName());
		adminReply.replace("%ordinal%", String.valueOf(player.getPlayerClass().getId()));
		adminReply.replace("%classid%", String.valueOf(player.getPlayerClass()));
		adminReply.replace("%baseclass%", ClassListData.getInstance().getClass(player.getBaseClass()).getClassName());
		adminReply.replace("%x%", String.valueOf(player.getX()));
		adminReply.replace("%y%", String.valueOf(player.getY()));
		adminReply.replace("%z%", String.valueOf(player.getZ()));
		adminReply.replace("%heading%", String.valueOf(player.getHeading()));
		adminReply.replace("%currenthp%", String.valueOf((int) player.getCurrentHp()));
		adminReply.replace("%maxhp%", String.valueOf(player.getMaxHp()));
		adminReply.replace("%reputation%", String.valueOf(player.getReputation()));
		adminReply.replace("%currentmp%", String.valueOf((int) player.getCurrentMp()));
		adminReply.replace("%maxmp%", String.valueOf(player.getMaxMp()));
		adminReply.replace("%pvpflag%", String.valueOf(player.getPvpFlag()));
		adminReply.replace("%currentcp%", String.valueOf((int) player.getCurrentCp()));
		adminReply.replace("%maxcp%", String.valueOf(player.getMaxCp()));
		adminReply.replace("%pvpkills%", String.valueOf(player.getPvpKills()));
		adminReply.replace("%pkkills%", String.valueOf(player.getPkKills()));
		adminReply.replace("%currentload%", String.valueOf(player.getCurrentLoad()));
		adminReply.replace("%maxload%", String.valueOf(player.getMaxLoad()));
		adminReply.replace("%percent%", String.format("%.2f", (((float) player.getCurrentLoad() / player.getMaxLoad()) * 100)));
		adminReply.replace("%patk%", String.valueOf(player.getPAtk()));
		adminReply.replace("%matk%", String.valueOf(player.getMAtk()));
		adminReply.replace("%pdef%", String.valueOf(player.getPDef()));
		adminReply.replace("%mdef%", String.valueOf(player.getMDef()));
		adminReply.replace("%accuracy%", String.valueOf(player.getAccuracy()));
		adminReply.replace("%evasion%", String.valueOf(player.getEvasionRate()));
		adminReply.replace("%critical%", String.valueOf(player.getCriticalHit()));
		adminReply.replace("%runspeed%", String.valueOf(player.getRunSpeed()));
		adminReply.replace("%patkspd%", String.valueOf(player.getPAtkSpd()));
		adminReply.replace("%matkspd%", String.valueOf(player.getMAtkSpd()));
		adminReply.replace("%hpregen%", String.valueOf(stat.getHpRegen()));
		adminReply.replace("%mpregen%", String.valueOf(stat.getMpRegen()));
		adminReply.replace("%cpregen%", String.valueOf(stat.getCpRegen()));
		adminReply.replace("%access%", player.getAccessLevel().getLevel() + " (" + player.getAccessLevel().getName() + ")");
		adminReply.replace("%account%", player.getAccountName());
		adminReply.replace("%ip%", ip);
		adminReply.replace("%protocol%", String.valueOf(player.getClient() != null ? player.getClient().getProtocolVersion() : "NULL"));
		adminReply.replace("%hwid%", (player.getClient() != null) && (player.getClient().getHardwareInfo() != null) ? player.getClient().getHardwareInfo().getMacAddress() : "Unknown");
		adminReply.replace("%ai%", player.getAI().getIntention().name());
		adminReply.replace("%autoplay%", AutoPlayConfig.ENABLE_AUTO_PLAY ? "<br1>AutoPlay: " + (player.isAutoPlaying() ? "Enabled" : "Disabled") : "");
		adminReply.replace("%inst%", player.isInInstance() ? " " + player.getInstanceWorld().getName() + "</td><td><button value=\"Go\" action=\"bypass -h admin_instanceteleport " + player.getInstanceId() + "\"width=60 height=20 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">" : "NONE");
		final int nobleLevel = player.getNobleLevel();
		adminReply.replace("%noblesse%", nobleLevel == 0 ? "No" : nobleLevel == 1 ? "Noble" : "Exalted");
		activeChar.sendPacket(adminReply);
	}
	
	private void setTargetReputation(Player activeChar, int value)
	{
		final WorldObject target = activeChar.getTarget();
		Player player = null;
		if (target.isPlayer())
		{
			player = target.asPlayer();
		}
		else
		{
			return;
		}
		
		int newReputation = value;
		if (newReputation > PvpConfig.MAX_REPUTATION)
		{
			newReputation = PvpConfig.MAX_REPUTATION;
		}
		
		final int oldReputation = player.getReputation();
		player.setReputation(newReputation);
		final SystemMessage sm = new SystemMessage(SystemMessageId.YOUR_REPUTATION_HAS_BEEN_CHANGED_TO_S1);
		sm.addInt(newReputation);
		player.sendPacket(sm);
		activeChar.sendSysMessage("Successfully Changed karma for " + player.getName() + " from (" + oldReputation + ") to (" + newReputation + ").");
	}
	
	private void editCharacter(Player activeChar, String targetName)
	{
		WorldObject target = null;
		if (targetName != null)
		{
			target = World.getInstance().getPlayer(targetName);
		}
		else
		{
			target = activeChar.getTarget();
		}
		
		if ((target != null) && target.isPlayer())
		{
			final Player player = target.asPlayer();
			gatherCharacterInfo(activeChar, player, "charedit.htm");
		}
	}
	
	/**
	 * @param activeChar
	 * @param characterToFind
	 */
	private void findCharacter(Player activeChar, String characterToFind)
	{
		int charactersFound = 0;
		String name;
		final NpcHtmlMessage adminReply = new NpcHtmlMessage(0, 1);
		adminReply.setFile(activeChar, "data/html/admin/charfind.htm");
		
		final StringBuilder replyMSG = new StringBuilder(1000);
		final List<Player> players = new ArrayList<>(World.getInstance().getPlayers());
		players.sort(Comparator.comparingLong(Player::getUptime));
		for (Player player : players)
		{ // Add player info into new Table row
			name = player.getName();
			if (name.toLowerCase().contains(characterToFind.toLowerCase()))
			{
				charactersFound += 1;
				replyMSG.append("<tr><td width=80><a action=\"bypass -h admin_character_info ");
				replyMSG.append(name);
				replyMSG.append("\">");
				replyMSG.append(name);
				replyMSG.append("</a></td><td width=110>");
				replyMSG.append(ClassListData.getInstance().getClass(player.getPlayerClass()).getClassName());
				replyMSG.append("</td><td width=40>");
				replyMSG.append(player.getLevel());
				replyMSG.append("</td></tr>");
			}
			
			if (charactersFound > 20)
			{
				break;
			}
		}
		
		adminReply.replace("%results%", replyMSG.toString());
		
		final String replyMSG2;
		if (charactersFound == 0)
		{
			replyMSG2 = "s. Please try again.";
		}
		else if (charactersFound > 20)
		{
			adminReply.replace("%number%", " more than 20");
			replyMSG2 = "s.<br>Please refine your search to see all of the results.";
		}
		else if (charactersFound == 1)
		{
			replyMSG2 = ".";
		}
		else
		{
			replyMSG2 = "s.";
		}
		
		adminReply.replace("%number%", String.valueOf(charactersFound));
		adminReply.replace("%end%", replyMSG2);
		activeChar.sendPacket(adminReply);
	}
	
	/**
	 * @param activeChar
	 * @param ipAdress
	 */
	private void findCharactersPerIp(Player activeChar, String ipAdress)
	{
		boolean findDisconnected = false;
		if (ipAdress.equals("disconnected"))
		{
			findDisconnected = true;
		}
		else if (!ipAdress.matches("^(?:(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2(?:[0-4][0-9]|5[0-5]))\\.){3}(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2(?:[0-4][0-9]|5[0-5]))$"))
		{
			throw new IllegalArgumentException("Malformed IPv4 number");
		}
		
		int charactersFound = 0;
		GameClient client;
		String ip = "0.0.0.0";
		final StringBuilder replyMSG = new StringBuilder(1000);
		final NpcHtmlMessage adminReply = new NpcHtmlMessage(0, 1);
		adminReply.setFile(activeChar, "data/html/admin/ipfind.htm");
		
		final List<Player> players = new ArrayList<>(World.getInstance().getPlayers());
		players.sort(Comparator.comparingLong(Player::getUptime));
		for (Player player : players)
		{
			client = player.getClient();
			if (client == null)
			{
				continue;
			}
			
			if (client.isDetached())
			{
				if (!findDisconnected)
				{
					continue;
				}
			}
			else
			{
				if (findDisconnected)
				{
					continue;
				}
				
				ip = client.getIp();
				if (!ip.equals(ipAdress))
				{
					continue;
				}
			}
			
			final String name = player.getName();
			charactersFound += 1;
			replyMSG.append("<tr><td width=80><a action=\"bypass -h admin_character_info ");
			replyMSG.append(name);
			replyMSG.append("\">");
			replyMSG.append(name);
			replyMSG.append("</a></td><td width=110>");
			replyMSG.append(ClassListData.getInstance().getClass(player.getPlayerClass()).getClassName());
			replyMSG.append("</td><td width=40>");
			replyMSG.append(player.getLevel());
			replyMSG.append("</td></tr>");
			
			if (charactersFound > 20)
			{
				break;
			}
		}
		
		adminReply.replace("%results%", replyMSG.toString());
		
		final String replyMSG2;
		if (charactersFound == 0)
		{
			replyMSG2 = "s. Maybe they got d/c? :)";
		}
		else if (charactersFound > 20)
		{
			adminReply.replace("%number%", " more than " + charactersFound);
			replyMSG2 = "s.<br>In order to avoid you a client crash I won't <br1>display results beyond the 20th character.";
		}
		else if (charactersFound == 1)
		{
			replyMSG2 = ".";
		}
		else
		{
			replyMSG2 = "s.";
		}
		
		adminReply.replace("%ip%", ipAdress);
		adminReply.replace("%number%", String.valueOf(charactersFound));
		adminReply.replace("%end%", replyMSG2);
		activeChar.sendPacket(adminReply);
	}
	
	/**
	 * @param activeChar
	 * @param characterName
	 */
	private void findCharactersPerAccount(Player activeChar, String characterName)
	{
		final Player player = World.getInstance().getPlayer(characterName);
		if (player == null)
		{
			throw new IllegalArgumentException("Player doesn't exist");
		}
		
		final Map<Integer, String> chars = player.getAccountChars();
		final StringJoiner replyMSG = new StringJoiner("<br1>");
		chars.values().stream().forEachOrdered(replyMSG::add);
		
		final NpcHtmlMessage adminReply = new NpcHtmlMessage(0, 1);
		adminReply.setFile(activeChar, "data/html/admin/accountinfo.htm");
		adminReply.replace("%account%", player.getAccountName());
		adminReply.replace("%player%", characterName);
		adminReply.replace("%characters%", replyMSG.toString());
		activeChar.sendPacket(adminReply);
	}
	
	/**
	 * @param activeChar
	 * @param multibox
	 */
	private void findDualbox(Player activeChar, int multibox)
	{
		final Map<String, List<Player>> ipMap = new HashMap<>();
		String ip = "0.0.0.0";
		GameClient client;
		final Map<String, Integer> dualboxIPs = new HashMap<>();
		final List<Player> players = new ArrayList<>(World.getInstance().getPlayers());
		players.sort(Comparator.comparingLong(Player::getUptime));
		for (Player player : players)
		{
			client = player.getClient();
			if ((client == null) || client.isDetached())
			{
				continue;
			}
			
			ip = client.getIp();
			if (ipMap.get(ip) == null)
			{
				ipMap.put(ip, new ArrayList<>());
			}
			
			ipMap.get(ip).add(player);
			
			if (ipMap.get(ip).size() >= multibox)
			{
				final Integer count = dualboxIPs.get(ip);
				if (count == null)
				{
					dualboxIPs.put(ip, multibox);
				}
				else
				{
					dualboxIPs.put(ip, count + 1);
				}
			}
		}
		
		final List<String> keys = new ArrayList<>(dualboxIPs.keySet());
		keys.sort(Comparator.comparing(dualboxIPs::get).reversed());
		
		final StringBuilder results = new StringBuilder();
		for (String dualboxIP : keys)
		{
			results.append("<a action=\"bypass -h admin_find_ip " + dualboxIP + "\">" + dualboxIP + " (" + dualboxIPs.get(dualboxIP) + ")</a><br1>");
		}
		
		final NpcHtmlMessage adminReply = new NpcHtmlMessage(0, 1);
		adminReply.setFile(activeChar, "data/html/admin/dualbox.htm");
		adminReply.replace("%multibox%", String.valueOf(multibox));
		adminReply.replace("%results%", results.toString());
		adminReply.replace("%strict%", "");
		activeChar.sendPacket(adminReply);
	}
	
	private void findDualboxStrict(Player activeChar, int multibox)
	{
		final Map<IpPack, List<Player>> ipMap = new HashMap<>();
		GameClient client;
		final Map<IpPack, Integer> dualboxIPs = new HashMap<>();
		final List<Player> players = new ArrayList<>(World.getInstance().getPlayers());
		players.sort(Comparator.comparingLong(Player::getUptime));
		for (Player player : players)
		{
			client = player.getClient();
			if ((client == null) || client.isDetached())
			{
				continue;
			}
			
			final IpPack pack = new IpPack(client.getIp(), client.getTrace());
			if (ipMap.get(pack) == null)
			{
				ipMap.put(pack, new ArrayList<>());
			}
			
			ipMap.get(pack).add(player);
			
			if (ipMap.get(pack).size() >= multibox)
			{
				final Integer count = dualboxIPs.get(pack);
				if (count == null)
				{
					dualboxIPs.put(pack, multibox);
				}
				else
				{
					dualboxIPs.put(pack, count + 1);
				}
			}
		}
		
		final List<IpPack> keys = new ArrayList<>(dualboxIPs.keySet());
		keys.sort(Comparator.comparing(dualboxIPs::get).reversed());
		
		final StringBuilder results = new StringBuilder();
		for (IpPack dualboxIP : keys)
		{
			results.append("<a action=\"bypass -h admin_find_ip " + dualboxIP.ip + "\">" + dualboxIP.ip + " (" + dualboxIPs.get(dualboxIP) + ")</a><br1>");
		}
		
		final NpcHtmlMessage adminReply = new NpcHtmlMessage(0, 1);
		adminReply.setFile(activeChar, "data/html/admin/dualbox.htm");
		adminReply.replace("%multibox%", String.valueOf(multibox));
		adminReply.replace("%results%", results.toString());
		adminReply.replace("%strict%", "strict_");
		activeChar.sendPacket(adminReply);
	}
	
	private class IpPack
	{
		String ip;
		int[][] tracert;
		
		public IpPack(String ip, int[][] tracert)
		{
			this.ip = ip;
			this.tracert = tracert;
		}
		
		@Override
		public int hashCode()
		{
			final int prime = 31;
			int result = 1;
			result = (prime * result) + ((ip == null) ? 0 : ip.hashCode());
			for (int[] array : tracert)
			{
				result = (prime * result) + Arrays.hashCode(array);
			}
			
			return result;
		}
		
		@Override
		public boolean equals(Object obj)
		{
			if (this == obj)
			{
				return true;
			}
			
			if (obj == null)
			{
				return false;
			}
			
			if (getClass() != obj.getClass())
			{
				return false;
			}
			
			final IpPack other = (IpPack) obj;
			if (!getOuterType().equals(other.getOuterType()))
			{
				return false;
			}
			
			if (ip == null)
			{
				if (other.ip != null)
				{
					return false;
				}
			}
			else if (!ip.equals(other.ip))
			{
				return false;
			}
			
			for (int i = 0; i < tracert.length; i++)
			{
				for (int o = 0; o < tracert[0].length; o++)
				{
					if (tracert[i][o] != other.tracert[i][o])
					{
						return false;
					}
				}
			}
			
			return true;
		}
		
		private AdminEditChar getOuterType()
		{
			return AdminEditChar.this;
		}
	}
	
	private void gatherSummonInfo(Summon target, Player activeChar)
	{
		final NpcHtmlMessage html = new NpcHtmlMessage(0, 1);
		html.setFile(activeChar, "data/html/admin/petinfo.htm");
		final String name = target.getName();
		html.replace("%name%", name == null ? "N/A" : name);
		html.replace("%level%", Integer.toString(target.getLevel()));
		html.replace("%exp%", Long.toString(target.getStat().getExp()));
		final String owner = target.asPlayer().getName();
		html.replace("%owner%", " <a action=\"bypass -h admin_character_info " + owner + "\">" + owner + "</a>");
		html.replace("%class%", target.getClass().getSimpleName());
		html.replace("%ai%", target.hasAI() ? target.getAI().getIntention().name() : "NULL");
		html.replace("%hp%", (int) target.getStatus().getCurrentHp() + "/" + target.getStat().getMaxHp());
		html.replace("%mp%", (int) target.getStatus().getCurrentMp() + "/" + target.getStat().getMaxMp());
		html.replace("%karma%", Integer.toString(target.getReputation()));
		html.replace("%race%", target.getTemplate().getRace().toString());
		if (target.isPet())
		{
			final int objId = target.asPlayer().getObjectId();
			html.replace("%inv%", " <a action=\"bypass admin_show_pet_inv " + objId + "\">view</a>");
		}
		else
		{
			html.replace("%inv%", "none");
		}
		
		if (target.isPet())
		{
			html.replace("%food%", target.asPet().getCurrentFed() + "/" + target.asPet().getPetLevelData().getPetMaxFeed());
			html.replace("%load%", target.getInventory().getTotalWeight() + "/" + target.getMaxLoad());
		}
		else
		{
			html.replace("%food%", "N/A");
			html.replace("%load%", "N/A");
		}
		
		activeChar.sendPacket(html);
	}
	
	private void gatherPartyInfo(Player target, Player activeChar)
	{
		boolean color = true;
		final NpcHtmlMessage html = new NpcHtmlMessage(0, 1);
		html.setFile(activeChar, "data/html/admin/partyinfo.htm");
		final StringBuilder text = new StringBuilder(400);
		
		final PartyDistributionType distributionType = target.getParty().getDistributionType();
		String lootInfo = "<tr><td><table width=290 border=0 cellpadding=0 bgcolor=000000>" + "<tr><td align=center><font color=\"LEVEL\">Loot:</font> " + (distributionType != null ? distributionType.name().replace('_', ' ') : "Unknown") + "</td></tr></table></td></tr>";
		String recallButton = "<button value=\"Recall Party\" action=\"bypass -h admin_recall_party_menu " + target.getName() + "\" width=\"80\" height=\"27\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">";
		
		// Header.
		text.append("<tr><td><table width=290 border=0 cellpadding=2 bgcolor=333333>");
		text.append("<tr>");
		text.append("<td width=30 align=center><font color=\"LEVEL\">Lvl</font></td>");
		text.append("<td width=20></td>");
		text.append("<td width=110><font color=\"LEVEL\">Name</font></td>");
		text.append("<td width=110 align=center><font color=\"LEVEL\">Class</font></td>");
		text.append("</tr></table></td></tr>");
		
		// Get party leader
		Player leader = target.getParty().getLeader();
		
		// Party members.
		for (Player member : target.getParty().getMembers())
		{
			text.append("<tr><td><table width=290 border=0 " + (color ? "bgcolor=000000 " : "") + "cellpadding=2><tr>");
			
			// Level.
			text.append("<td width=30 align=center>" + member.getLevel() + "</td>");
			
			// Show icon if member is leader.
			if (member == leader)
			{
				text.append("<td width=20 align=center><img src=\"L2UI_CH3.party_leadericon\" width=12 height=12></td>");
			}
			else
			{
				text.append("<td width=20></td>");
			}
			
			// Name and class.
			text.append("<td width=110><a action=\"bypass -h admin_character_info ").append(member.getName()).append("\">").append(member.getName()).append("</a></td>");
			
			text.append("<td width=110 align=center>").append(ClassListData.getInstance().getClass(member.getPlayerClass()).getClassName()).append("</td>");
			text.append("</tr></table></td></tr>");
			
			color = !color;
		}
		
		html.replace("%party_loot%", lootInfo);
		html.replace("%player%", target.getName());
		html.replace("%party%", text.toString());
		html.replace("%recall_party%", recallButton);
		
		activeChar.sendPacket(html);
	}
}
