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
package ai.others.ClanHallManager;

import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.StringTokenizer;

import org.l2jmobius.commons.util.StringUtil;
import org.l2jmobius.gameserver.data.xml.ResidenceFunctionsData;
import org.l2jmobius.gameserver.data.xml.TeleporterData;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Merchant;
import org.l2jmobius.gameserver.model.clan.ClanAccess;
import org.l2jmobius.gameserver.model.clan.enums.ClanHallGrade;
import org.l2jmobius.gameserver.model.residences.ClanHall;
import org.l2jmobius.gameserver.model.residences.ResidenceFunction;
import org.l2jmobius.gameserver.model.residences.ResidenceFunctionTemplate;
import org.l2jmobius.gameserver.model.residences.ResidenceFunctionType;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.teleporter.TeleportHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.AgitDecoInfo;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Clan Hall Manager AI.
 * @author St3eT
 */
public class ClanHallManager extends Script
{
	// NPCs
	// @formatter:off
	private static final int[] CLANHALL_MANAGERS =
	{
		35384, 35386, 35388, 35390, // Gludio
		35400, 35392, 35394, 35396, 35398, // Gludin
		35403, 35405, 35407, // Dion
		35439, 35441, 35443, 35445, 35447, 35449, // Aden
		35451, 35453, 35455, 35457, 35459, // Giran
		35461, 35463, 35465, 35467, // Goddard
		35566, 35568, 35570, 35572, 35574, 35576, 35578, // Rune
		35580, 35582, 35584, 35586, // Schuttgart
		36721, 36723, 36725, 36727, // Gludio Outskirts
		36729, 36731, 36733, 36735, // Dion Outskirts
		36737, 36739, // Floran Village
		33360, // Talking Island
	};
	// @formatter:on
	
	// Misc
	private static final int[] ALLOWED_BUFFS =
	{
		4342, // Wind Walk
		4343, // Decrease Weight
		4344, // Shield
		4346, // Mental Shield
		4345, // Might
		15374, // Horn Melody
		15375, // Drum Melody
		4347, // Blessed Body
		4349, // Magic Barrier
		4350, // Resist Shock
		4348, // Blessed Soul
		15376, // Pipe Organ Melody
		15377, // Guitar Melody
		4351, // Concentration
		4352, // Berserker Spirit
		4353, // Blessed Shield
		4358, // Guidance
		4354, // Vampiric Rage
		15380, // Knight's Harmony
		15381, // Warrior's Harmony
		15382, // Wizard's Harmony
		4355, // Acumen
		4356, // Empower
		4357, // Haste
		4359, // Focus
		4360, // Death Whisper
	};
	
	private ClanHallManager()
	{
		addStartNpc(CLANHALL_MANAGERS);
		addTalkId(CLANHALL_MANAGERS);
		addFirstTalkId(CLANHALL_MANAGERS);
		addCreatureSeeId(CLANHALL_MANAGERS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final StringTokenizer st = new StringTokenizer(event, " ");
		final String action = st.nextToken();
		final ClanHall clanHall = npc.getClanHall();
		String htmltext = null;
		if ((clanHall != null) && isOwningClan(player, npc))
		{
			switch (action)
			{
				case "index":
				{
					if (isOwningClan(player, npc))
					{
						if (npc.getId() == 33360) // Provisional Hall Manager
						{
							htmltext = "ClanHallManager-01b.html";
						}
						else if (clanHall.getCostFailDay() == 0)
						{
							htmltext = "ClanHallManager-01.html";
						}
					}
					else
					{
						htmltext = "ClanHallManager-03.html";
					}
					break;
				}
				case "manageDoors":
				{
					if (player.hasAccess(ClanAccess.HALL_OPEN_DOOR))
					{
						if (st.hasMoreTokens())
						{
							final boolean open = st.nextToken().equals("1");
							clanHall.openCloseDoors(open);
							htmltext = "ClanHallManager-0" + (open ? "5" : "6") + ".html";
						}
						else
						{
							htmltext = "ClanHallManager-04.html";
						}
					}
					else
					{
						htmltext = "ClanHallManager-noAuthority.html";
					}
					break;
				}
				case "expel":
				{
					if (player.hasAccess(ClanAccess.HALL_BANISH))
					{
						if (st.hasMoreTokens())
						{
							clanHall.banishOthers();
							htmltext = "ClanHallManager-08.html";
						}
						else
						{
							htmltext = "ClanHallManager-07.html";
						}
					}
					else
					{
						htmltext = "ClanHallManager-noAuthority.html";
					}
					break;
				}
				case "useFunctions":
				{
					if (player.hasAccess(ClanAccess.HALL_FUNCTIONS))
					{
						if (!st.hasMoreTokens())
						{
							final ResidenceFunction hpFunc = clanHall.getFunction(ResidenceFunctionType.HP_REGEN);
							final ResidenceFunction mpFunc = clanHall.getFunction(ResidenceFunctionType.MP_REGEN);
							final ResidenceFunction xpFunc = clanHall.getFunction(ResidenceFunctionType.EXP_RESTORE);
							htmltext = getHtm(player, "ClanHallManager-09.html");
							htmltext = htmltext.replace("%hpFunction%", hpFunc != null ? String.format("%d%%", Math.round((hpFunc.getValue() - 1) * 100)) : "0");
							htmltext = htmltext.replace("%mpFunction%", mpFunc != null ? String.format("%d%%", Math.round((mpFunc.getValue() - 1) * 100)) : "0");
							htmltext = htmltext.replace("%resFunction%", xpFunc != null ? String.format("%d%%", (int) xpFunc.getValue()) : "0");
						}
						else
						{
							switch (st.nextToken())
							{
								case "teleport":
								{
									final int teleportLevel = clanHall.getFunctionLevel(ResidenceFunctionType.TELEPORT);
									if (teleportLevel > 0)
									{
										final TeleportHolder holder = TeleporterData.getInstance().getHolder(npc.getId(), "tel" + teleportLevel);
										if (holder != null)
										{
											if (!st.hasMoreTokens())
											{
												holder.showTeleportList(player, npc, "Script ClanHallManager useFunctions teleport");
											}
											else if (st.countTokens() >= 2)
											{
												final String listName = st.nextToken();
												final int funcLvl = (listName.length() >= 4) ? StringUtil.parseInt(listName.substring(3), -1) : -1;
												if (teleportLevel == funcLvl)
												{
													holder.doTeleport(player, npc, StringUtil.parseNextInt(st, -1));
												}
											}
										}
									}
									else
									{
										htmltext = "ClanHallManager-noFunction.html";
									}
									break;
								}
								case "buffs":
								{
									final int buffLevel = clanHall.getFunctionLevel(ResidenceFunctionType.BUFF);
									if (buffLevel > 0)
									{
										if (!st.hasMoreTokens())
										{
											htmltext = getHtm(player, "ClanHallManager-funcBuffs_" + buffLevel + ".html");
											htmltext = htmltext.replace("%manaLeft%", Integer.toString((int) npc.getCurrentMp()));
										}
										else
										{
											final String[] skillData = st.nextToken().split("_");
											final SkillHolder skill = new SkillHolder(Integer.parseInt(skillData[0]), Integer.parseInt(skillData[1]));
											if (ArrayUtil.contains(ALLOWED_BUFFS, skill.getSkillId()))
											{
												if (npc.getCurrentMp() < (npc.getStat().getMpConsume(skill.getSkill()) + npc.getStat().getMpInitialConsume(skill.getSkill())))
												{
													htmltext = getHtm(player, "ClanHallManager-funcBuffsNoMp.html");
												}
												else if (npc.isSkillDisabled(skill.getSkill()))
												{
													htmltext = getHtm(player, "ClanHallManager-funcBuffsNoReuse.html");
												}
												else
												{
													castSkill(npc, player, skill);
													htmltext = getHtm(player, "ClanHallManager-funcBuffsDone.html");
												}
												
												htmltext = htmltext.replace("%manaLeft%", Integer.toString((int) npc.getCurrentMp()));
											}
										}
									}
									else
									{
										htmltext = "ClanHallManager-noFunction.html";
									}
									break;
								}
								case "items":
								{
									final int itemLevel = clanHall.getFunctionLevel(ResidenceFunctionType.ITEM);
									switch (itemLevel)
									{
										case 1:
										case 2:
										case 3:
										{
											if (npc.getId() == 33360) // Provisional Hall Manager
											{
												((Merchant) npc).showBuyWindow(player, Integer.parseInt(clanHall.getResidenceId() + "0" + (itemLevel - 1)));
											}
											else
											{
												((Merchant) npc).showBuyWindow(player, Integer.parseInt(npc.getId() + "0" + (itemLevel - 1)));
											}
											break;
										}
										default:
										{
											htmltext = "ClanHallManager-noFunction.html";
										}
									}
									break;
								}
							}
						}
					}
					else
					{
						htmltext = "ClanHallManager-noAuthority.html";
					}
					break;
				}
				case "warehouse":
				{
					htmltext = getHtm(player, "ClanHallManager-10.html");
					htmltext = htmltext.replace("%lease%", String.valueOf(clanHall.getLease()));
					htmltext = htmltext.replace("%payDate%", new SimpleDateFormat("yyyy-MM-dd HH:mm:ss").format(new Date(clanHall.getNextPayment())));
					break;
				}
				case "manageFunctions":
				{
					if (player.hasAccess(ClanAccess.HALL_MANAGE_FUNCTIONS))
					{
						if (!st.hasMoreTokens())
						{
							if (npc.getId() == 33360) // Provisional Hall Manager
							{
								htmltext = "ClanHallManager-11b.html";
							}
							else
							{
								htmltext = "ClanHallManager-11.html";
							}
						}
						else
						{
							switch (st.nextToken())
							{
								case "recovery":
								{
									htmltext = getHtm(player, clanHall.getGrade() == ClanHallGrade.GRADE_S ? "ClanHallManager-manageFuncRecoverySGrade.html" : "ClanHallManager-manageFuncRecoveryBGrade.html");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.HP_REGEN), htmltext, "HP");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.MP_REGEN), htmltext, "MP");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.EXP_RESTORE), htmltext, "XP");
									break;
								}
								case "other":
								{
									htmltext = getHtm(player, "ClanHallManager-manageFuncOther.html");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.TELEPORT), htmltext, "TP");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.BUFF), htmltext, "BUFF");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.ITEM), htmltext, "ITEM");
									break;
								}
								case "decor":
								{
									htmltext = getHtm(player, "ClanHallManager-manageFuncDecor.html");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.CURTAIN), htmltext, "CURTAIN");
									htmltext = getFunctionInfo(clanHall.getFunction(ResidenceFunctionType.PLATFORM), htmltext, "PODIUM");
									break;
								}
								case "selectFunction":
								{
									if (st.countTokens() == 2)
									{
										final int funcId = Integer.parseInt(st.nextToken());
										final int funcLv = Integer.parseInt(st.nextToken());
										final ResidenceFunction oldFunc = clanHall.getFunction(funcId, funcLv);
										if (oldFunc != null)
										{
											final int funcVal = (int) oldFunc.getTemplate().getValue();
											htmltext = getHtm(player, "ClanHallManager-manageFuncAlreadySet.html");
											htmltext = htmltext.replace("%funcEffect%", "<fstring p1=\"" + (funcVal > 0 ? funcVal : oldFunc.getLevel()) + "\">" + (funcVal > 0 ? NpcStringId.S1.getId() : NpcStringId.STAGE_S1.getId()) + "</fstring>");
										}
										else if ((funcId >= 1) && (funcId <= 8))
										{
											final ResidenceFunctionTemplate template = ResidenceFunctionsData.getInstance().getFunction(funcId, funcLv);
											if (template != null)
											{
												htmltext = getHtm(player, "ClanHallManager-funcConfirm" + funcId + ".html");
												htmltext = htmltext.replace("%funcId%", String.valueOf(funcId));
												htmltext = htmltext.replace("%funcLv%", String.valueOf(funcLv));
												htmltext = htmltext.replace("%funcCost%", "<fstring p1=\"" + template.getCost().getCount() + "\" p2=\"" + template.getDurationAsDays() + "\">" + NpcStringId.FONT_COLOR_FFAABB_S1_FONT_ADENA_S2_DAY_S.getId() + "</fstring>");
											}
										}
									}
									break;
								}
								case "setFunction":
								{
									if (st.countTokens() == 2)
									{
										final int funcId = Integer.parseInt(st.nextToken());
										final int funcLv = Integer.parseInt(st.nextToken());
										final ResidenceFunctionTemplate template = ResidenceFunctionsData.getInstance().getFunction(funcId, funcLv);
										if ((template != null) && (getQuestItemsCount(player, template.getCost().getId()) >= template.getCost().getCount()))
										{
											if (clanHall.getFunction(funcId, funcLv) != null)
											{
												return null;
											}
											
											takeItems(player, template.getCost().getId(), template.getCost().getCount());
											clanHall.addFunction(funcId, funcLv);
											updateVisualEffects(clanHall, npc);
											htmltext = "ClanHallManager-manageFuncDone.html";
										}
										else
										{
											htmltext = "ClanHallManager-noAdena.html";
										}
									}
									break;
								}
								case "removeFunction":
								{
									if (st.countTokens() == 2)
									{
										final String act = st.nextToken();
										final ResidenceFunctionType funcType = ResidenceFunctionType.valueOf(st.nextToken());
										if (funcType != null)
										{
											if (act.equals("confirm"))
											{
												htmltext = getHtm(player, "ClanHallManager-removeFunctionConfirm.html");
												htmltext = htmltext.replace("%FUNC_TYPE%", funcType.toString());
											}
											else if (act.equals("remove"))
											{
												final ResidenceFunction func = clanHall.getFunction(funcType);
												if (func != null)
												{
													clanHall.removeFunction(func);
													updateVisualEffects(clanHall, npc);
													htmltext = "ClanHallManager-removeFunctionDone.html";
												}
												else
												{
													htmltext = "ClanHallManager-removeFunctionFail.html";
												}
											}
											else
											{
												htmltext = "ClanHallManager-removeFunctionFail.html";
											}
										}
										else
										{
											htmltext = "ClanHallManager-removeFunctionFail.html";
										}
									}
									else
									{
										htmltext = "ClanHallManager-removeFunctionFail.html";
									}
									break;
								}
							}
						}
					}
					else
					{
						htmltext = "ClanHallManager-noAuthority.html";
					}
					break;
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = null;
		final ClanHall clanHall = npc.getClanHall();
		if (isOwningClan(player, npc))
		{
			if (npc.getId() == 33360) // Provisional Hall Manager
			{
				htmltext = "ClanHallManager-01b.html";
			}
			else if (clanHall.getCostFailDay() == 0)
			{
				htmltext = "ClanHallManager-01.html";
			}
			else
			{
				htmltext = getHtm(player, "ClanHallManager-02.html");
				htmltext = htmltext.replace("%costFailDayLeft%", Integer.toString((8 - clanHall.getCostFailDay())));
			}
		}
		else
		{
			htmltext = "ClanHallManager-03.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onCreatureSee(Npc npc, Creature creature)
	{
		if (creature.isPlayer())
		{
			final ClanHall clanHall = npc.getClanHall();
			if (clanHall != null)
			{
				creature.asPlayer().sendPacket(new AgitDecoInfo(clanHall));
			}
		}
	}
	
	private void updateVisualEffects(ClanHall clanHall, Npc npc)
	{
		World.getInstance().forEachVisibleObject(npc, Player.class, player -> player.sendPacket(new AgitDecoInfo(clanHall)));
	}
	
	private String getFunctionInfo(ResidenceFunction func, String htmltextValue, String name)
	{
		String htmltext = htmltextValue;
		if (func != null)
		{
			htmltext = htmltext.replaceAll("%" + name + "recovery%", String.valueOf((int) func.getTemplate().getValue()) + "%");
			htmltext = htmltext.replaceAll("%" + name + "price%", "<fstring p1=\"" + func.getTemplate().getCost().getCount() + "\" p2=\"" + func.getTemplate().getDurationAsDays() + "\">" + NpcStringId.FONT_COLOR_FFAABB_S1_FONT_ADENA_S2_DAY_S.getId() + "</fstring>");
			htmltext = htmltext.replace("%" + name + "expire%", "Withdraw the fee for the next time at " + new SimpleDateFormat("yyyy-MM-dd HH:mm:ss").format(new Date(func.getExpiration())));
			htmltext = htmltext.replaceAll("%" + name + "deactive%", "[<a action=\"bypass -h Script ClanHallManager manageFunctions removeFunction confirm " + func.getType().toString() + "\">Deactivate</a>]");
		}
		else
		{
			htmltext = htmltext.replaceAll("%" + name + "recovery%", "<fstring p1=\"\" p2=\"\">" + NpcStringId.NONE_2.getId() + "</fstring>");
			htmltext = htmltext.replaceAll("%" + name + "price%", "");
			htmltext = htmltext.replaceAll("%" + name + "expire%", "");
			htmltext = htmltext.replaceAll("%" + name + "deactive%", "");
		}
		
		return htmltext;
	}
	
	private boolean isOwningClan(Player player, Npc npc)
	{
		return ((npc.getClanHall().getOwnerId() == player.getClanId()) && (player.getClanId() != 0));
	}
	
	public static void main(String[] args)
	{
		new ClanHallManager();
	}
}
