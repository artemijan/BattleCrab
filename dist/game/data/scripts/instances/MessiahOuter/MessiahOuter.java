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
package instances.MessiahOuter;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Random;
import java.util.Set;

import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.model.zone.type.SayuneZone;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.util.ArrayUtil;

/**
 * Messiah Castle Outer Fort.
 * @author Tanatos
 */
public class MessiahOuter extends InstanceScript
{
	// NPCs
	private static final int ERDA = 34319;
	private static final int LEONA = 34301;
	private static final int ELIKIA = 34302;
	private static final int DEVIANNE = 34303;
	private static final int SPORCHA = 34321;
	private static final int TELEPORTER = 34325;
	private static final int VIOLET = 34327;
	private static final int SOLDIER = 34300;
	
	// Monsters
	private static final int SIEGE_TANK = 24023;
	private static final int MAGIC_CANNON = 24042;
	private static final int[] MONSTERS =
	{
		24014,
		24015,
		24016,
		24017,
		24018,
		24019,
		24020,
		24021,
		24022,
		SIEGE_TANK,
		MAGIC_CANNON,
		24043,
		24044,
		24045,
		24046,
		24047,
		24048,
		24049,
	};
	private static final int CASTLE_RAIDER = 26230;
	private static final int POSLOF = 26231;
	private static final int RESURRECTED_CASTLE_ARCHER = 26232;
	private static final int ORC_MERCENARY = 26233;
	private static final int CASTLE_WIZARD = 26234;
	private static final int[] MINIONS =
	{
		CASTLE_RAIDER,
		POSLOF,
		RESURRECTED_CASTLE_ARCHER,
		ORC_MERCENARY,
		CASTLE_WIZARD
	};
	
	// Raid Bosses
	private static final int SAMPSON = 26199;
	private static final int HANSON = 26200;
	private static final int GROM = 26201;
	private static final int MEDVEZ = 26202;
	private static final int ZIGATAN = 26203;
	private static final int HUNCHBACK_KWAI = 26204;
	private static final int CORNIX = 26205;
	private static final int CARANIX = 26206;
	private static final int JONADAN = 26207;
	private static final int DEMIEN = 26208;
	private static final int BERK = 26209;
	private static final int TARKU = 26210;
	private static final int TARPIN = 26211;
	private static final int SAFE_VAULT = 26212;
	private static final int SECRET_VAULT = 26213;
	private static final int SAKUM = 26214;
	private static final int CRAZY_TYPHOON = 26215;
	private static final int CURSED_HAREN = 26216;
	private static final int FLYNT = 26217;
	private static final int HARP = 26218;
	private static final int MALISS = 26219;
	private static final int ISADORA = 26220;
	private static final int WHITRA = 26221;
	private static final int BLETRA = 26222;
	private static final int UPGRADED_SIEGE_TANK = 26223;
	private static final int VEGIMA = 26224;
	private static final int VARONIA = 26225;
	private static final int ARONIA = 26226;
	private static final int ODD = 26227;
	private static final int EVEN = 26228;
	private static final int NEMERTESS = 26229;
	private static final int[] BOSSES =
	{
		SAMPSON,
		HANSON,
		GROM,
		MEDVEZ,
		ZIGATAN,
		HUNCHBACK_KWAI,
		CORNIX,
		CARANIX,
		JONADAN,
		DEMIEN,
		BERK,
		TARKU,
		TARPIN,
		SAFE_VAULT,
		SECRET_VAULT,
		SAKUM,
		CRAZY_TYPHOON,
		CURSED_HAREN,
		FLYNT,
		HARP,
		MALISS,
		ISADORA,
		WHITRA,
		BLETRA,
		UPGRADED_SIEGE_TANK,
		VEGIMA,
		VARONIA,
		ARONIA,
		ODD,
		EVEN,
		NEMERTESS
	};
	
	// Skills
	private static final SkillHolder MASS_CHAIN_SMASH = new SkillHolder(16154, 1);
	private static final SkillHolder MASS_BLEED = new SkillHolder(16225, 1);
	private static final SkillHolder WIDE_STUN = new SkillHolder(16228, 1);
	private static final SkillHolder CRITICAL_SLASHER = new SkillHolder(16763, 1);
	private static final SkillHolder FATAL_STRIKE = new SkillHolder(16764, 1);
	private static final SkillHolder FATAL_SLASHER = new SkillHolder(16766, 1);
	private static final SkillHolder ENERGY_SPIKE = new SkillHolder(16768, 1);
	private static final SkillHolder ENERGY_BURST = new SkillHolder(16769, 1);
	private static final SkillHolder ENERGY_BLAST = new SkillHolder(16770, 1);
	private static final SkillHolder EMBRYO_BERSERKER = new SkillHolder(16889, 1);
	private static final SkillHolder EMBRYO_GUARDIAN = new SkillHolder(16890, 1);
	private static final SkillHolder[] CANNON_GROUP =
	{
		ENERGY_SPIKE,
		ENERGY_BLAST
	};
	private static final SkillHolder[] VAULT_GROUP =
	{
		CRITICAL_SLASHER,
		ENERGY_BURST
	};
	private static final SkillHolder[] FIGHTER_GROUP =
	{
		MASS_CHAIN_SMASH,
		MASS_BLEED,
		WIDE_STUN,
		CRITICAL_SLASHER,
		FATAL_STRIKE,
		FATAL_SLASHER
	};
	private static final SkillHolder[] MAGE_GROUP =
	{
		CRITICAL_SLASHER,
		ENERGY_SPIKE,
		ENERGY_BURST
	};
	
	// Locations
	private static final Location VIOLET_LOC = new Location(-248896, 132589, 1051);
	
	// Zones
	private static final int DEVIANNE_1 = 9000;
	private static final int DEVIANNE_2 = 9001;
	private static final int DEVIANNE_3 = 9002;
	private static final int ELIKIA_1 = 9003;
	private static final int ELIKIA_2 = 9004;
	private static final int ELIKIA_3 = 9005;
	private static final int SPORCHA_1 = 9006;
	private static final int SPORCHA_2 = 9007;
	private static final int SPORCHA_3 = 9008;
	private static final int LEONA_1 = 9009;
	private static final int LEONA_2 = 9010;
	private static final int LEONA_3 = 9011;
	private static final SayuneZone DEVIANNE_SAYUNE_1 = ZoneManager.getInstance().getZoneByName("Sayune_87", SayuneZone.class);
	private static final SayuneZone DEVIANNE_SAYUNE_2 = ZoneManager.getInstance().getZoneByName("Sayune_88", SayuneZone.class);
	private static final SayuneZone DEVIANNE_SAYUNE_3 = ZoneManager.getInstance().getZoneByName("Sayune_89", SayuneZone.class);
	private static final SayuneZone ELIKIA_SAYUNE_1 = ZoneManager.getInstance().getZoneByName("Sayune_90", SayuneZone.class);
	private static final SayuneZone ELIKIA_SAYUNE_2 = ZoneManager.getInstance().getZoneByName("Sayune_91", SayuneZone.class);
	private static final SayuneZone ELIKIA_SAYUNE_3 = ZoneManager.getInstance().getZoneByName("Sayune_92", SayuneZone.class);
	private static final SayuneZone SPORCHA_SAYUNE_1 = ZoneManager.getInstance().getZoneByName("Sayune_93", SayuneZone.class);
	private static final SayuneZone SPORCHA_SAYUNE_2 = ZoneManager.getInstance().getZoneByName("Sayune_94", SayuneZone.class);
	private static final SayuneZone SPORCHA_SAYUNE_3 = ZoneManager.getInstance().getZoneByName("Sayune_95", SayuneZone.class);
	private static final SayuneZone LEONA_SAYUNE_1 = ZoneManager.getInstance().getZoneByName("Sayune_96", SayuneZone.class);
	private static final SayuneZone LEONA_SAYUNE_2 = ZoneManager.getInstance().getZoneByName("Sayune_97", SayuneZone.class);
	private static final SayuneZone LEONA_SAYUNE_3 = ZoneManager.getInstance().getZoneByName("Sayune_98", SayuneZone.class);
	
	// Misc
	private static final int TEMPLATE_ID = 265;
	private static final int COND_MEMBERS = 21;
	private static final int COND_LEVEL = 100;
	final Set<Integer> bossSet = new HashSet<>();
	
	public MessiahOuter()
	{
		super(TEMPLATE_ID);
		addInstanceCreatedId(TEMPLATE_ID);
		addStartNpc(ERDA);
		addFirstTalkId(LEONA, ELIKIA, DEVIANNE, SPORCHA, TELEPORTER, VIOLET);
		addTalkId(LEONA, ELIKIA, DEVIANNE, SPORCHA, TELEPORTER, VIOLET);
		addSpawnId(SOLDIER);
		addSpawnId(MONSTERS);
		addSpawnId(BOSSES);
		addAttackId(BOSSES);
		addKillId(SIEGE_TANK, MAGIC_CANNON);
		addKillId(BOSSES);
		addEnterZoneId(DEVIANNE_1, DEVIANNE_2, DEVIANNE_3, ELIKIA_1, ELIKIA_2, ELIKIA_3, SPORCHA_1, SPORCHA_2, SPORCHA_3, LEONA_1, LEONA_2, LEONA_3);
	}
	
	@Override
	public void onInstanceCreated(Instance instance, Player player)
	{
		instance.getParameters().getInt("ALIVE_RAIDS", 12);
		
		for (int boss : BOSSES)
		{
			bossSet.add(boss);
		}
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "enterInstance":
			{
				if (player.isInCommandChannel())
				{
					final List<Player> members = player.getCommandChannel().getMembers();
					if (members.size() >= COND_MEMBERS)
					{
						for (Player member : members)
						{
							if (member.getLevel() >= COND_LEVEL)
							{
								if (!member.isInsideRadius3D(npc, 1000))
								{
									player.sendMessage("Player " + member.getName() + " must come closer.");
								}
								
								enterInstance(member, npc, TEMPLATE_ID);
							}
							else
							{
								htmltext = "condNoLevel.html";
								return htmltext;
							}
						}
					}
					else
					{
						htmltext = "condNoCommandChannel.html";
						return htmltext;
					}
				}
				else if (player.isGM())
				{
					enterInstance(player, npc, TEMPLATE_ID);
				}
				else
				{
					htmltext = "condNoCommandChannel.html";
					return htmltext;
				}
				break;
			}
			case "toViolet":
			{
				final Instance instance = player.getInstanceWorld();
				if (isInInstance(instance))
				{
					player.teleToLocation(VIOLET_LOC);
				}
				break;
			}
			case "soldierShoutRefresh":
			{
				final Instance instance = npc.getInstanceWorld();
				if (isInInstance(instance))
				{
					instance.getParameters().set("SOLDIER_SHOUT", false);
				}
				break;
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		if (instance != null)
		{
			switch (npc.getId())
			{
				case SOLDIER:
				{
					final boolean soldierShout = instance.getParameters().getBoolean("SOLDIER_SHOUT", false);
					if (!soldierShout)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WE_LL_GUARD_THIS_PLACE_USE_THIS_SAYUNE_TO_TELEPORT_TO_THE_NEXT_AREA);
						instance.getParameters().set("SOLDIER_SHOUT", true);
						startQuestTimer("soldierShoutRefresh", 5000, npc, null);
					}
					break;
				}
				case SAMPSON:
				{
					showOnScreenMsg(instance, NpcStringId.GUARD_CAPTAIN_SAMPSON_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.STOP_THE_INVADERS);
					break;
				}
				case HANSON:
				{
					showOnScreenMsg(instance, NpcStringId.GUARD_CAPTAIN_HANSON_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.STOP_THE_INVADERS);
					break;
				}
				case GROM:
				{
					showOnScreenMsg(instance, NpcStringId.DESTROYER_GROM_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.MY_BLOOD_BOILS_I_M_GAME);
					break;
				}
				case MEDVEZ:
				{
					showOnScreenMsg(instance, NpcStringId.EXECUTOR_MEDVEZ, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.INVADERS_DESTROY);
					break;
				}
				case ZIGATAN:
				{
					showOnScreenMsg(instance, NpcStringId.ZIGATAN_THE_DARK_KNIGHT_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_HAVE_RETURNED_BY_THE_POWER_OF_DARKNESS);
					break;
				}
				case HUNCHBACK_KWAI:
				{
					showOnScreenMsg(instance, NpcStringId.HUNCHBACK_KWAI_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.FOR_ETIS_VAN_ETINA_OUR_LIVING_GOD);
					break;
				}
				case CORNIX:
				{
					showOnScreenMsg(instance, NpcStringId.SOUL_REAPER_CORNIX_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ALL_YOUR_SOULS_ARE_MINE);
					break;
				}
				case CARANIX:
				{
					showOnScreenMsg(instance, NpcStringId.SOUL_REAPER_CARANIX_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ALL_YOUR_SOULS_ARE_MINE);
					break;
				}
				case JONADAN:
				{
					showOnScreenMsg(instance, NpcStringId.CREED_GUARDIAN_JONADAN_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HEATHENS_YOU_RE_DESECRATING_THE_HOLY_GROUND);
					break;
				}
				case DEMIEN:
				{
					showOnScreenMsg(instance, NpcStringId.CREED_GUARDIAN_DEMIEN_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HEATHENS_YOU_RE_DESECRATING_THE_HOLY_GROUND);
					break;
				}
				case BERK:
				{
					showOnScreenMsg(instance, NpcStringId.HIGH_EXECUTOR_BERG_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.AARRGGHH_I_WILL_CRUSH_THEM_LIKE_BUGS);
					break;
				}
				case TARKU:
				{
					showOnScreenMsg(instance, NpcStringId.TARKU_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HAHAHA_WHAT_A_GREAT_PICK_ME_UP);
					break;
				}
				case TARPIN:
				{
					showOnScreenMsg(instance, NpcStringId.TARPIN_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HAHAHA_WHAT_A_GREAT_PICK_ME_UP);
					break;
				}
				case SAFE_VAULT:
				{
					showOnScreenMsg(instance, NpcStringId.EMBRYO_SAFE_VAULT_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THREATS_DETECTED_ACTIVATING_THE_SELF_DEFENSE_SYSTEM);
					break;
				}
				case SECRET_VAULT:
				{
					showOnScreenMsg(instance, NpcStringId.EMBRYO_SECRET_VAULT_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THREATS_DETECTED_ACTIVATING_THE_SELF_DEFENSE_SYSTEM);
					break;
				}
				case SAKUM:
				{
					showOnScreenMsg(instance, NpcStringId.FINAL_EVOLUTION_FORM_SAKUM_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.KRRR);
					break;
				}
				case CRAZY_TYPHOON:
				{
					showOnScreenMsg(instance, NpcStringId.CRAZY_TYPHOON_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WAHAHA_BLOOD_WILL_BE_SPILT);
					break;
				}
				case CURSED_HAREN:
				{
					showOnScreenMsg(instance, NpcStringId.CURSED_HAREN_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.KELBIM_WHERE_ARE_YOU_YOU_WILL_PAY_FOR_YOUR_DEEDS);
					break;
				}
				case FLYNT:
				{
					showOnScreenMsg(instance, NpcStringId.EVIL_DESTRUCTOR_FLYNT_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.AARRGGHHH);
					break;
				}
				case HARP:
				{
					showOnScreenMsg(instance, NpcStringId.PERFECT_HARP_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.INITIATE_ANNIHILATION_PROCEDURE);
					break;
				}
				case MALISS:
				{
					showOnScreenMsg(instance, NpcStringId.GRAND_PRIEST_OF_BLOOD_MALISS_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.COME_ENEMIES_OF_ETIS_VAN_ETINA_WE_SHALL_PREVAIL);
					break;
				}
				case ISADORA:
				{
					showOnScreenMsg(instance, NpcStringId.ISADORA_THE_GREAT_MAGUS_OF_GREED_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.REMOVE_YOUR_PAWS_FROM_MALISS);
					break;
				}
				case WHITRA:
				{
					showOnScreenMsg(instance, NpcStringId.GUARDIAN_WHITERA_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_IS_IT_LEONA_S_HOUNDS);
					break;
				}
				case BLETRA:
				{
					showOnScreenMsg(instance, NpcStringId.GUARDIAN_BLETRA_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_IS_IT_LEONA_S_HOUNDS);
					break;
				}
				case UPGRADED_SIEGE_TANK:
				{
					showOnScreenMsg(instance, NpcStringId.ADVANCED_SIEGE_TANK_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DEPLOYMENT_COMPLETE_ENTERING_COMBAT);
					break;
				}
				case VEGIMA:
				{
					showOnScreenMsg(instance, NpcStringId.VEGIMA_THE_DARK_SHAMAN_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_HAVE_RETURNED_BY_THE_POWER_OF_DARKNESS);
					break;
				}
				case VARONIA:
				{
					showOnScreenMsg(instance, NpcStringId.SOUL_REAPER_VARONIA_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ALL_YOUR_SOULS_ARE_MINE);
					break;
				}
				case ARONIA:
				{
					showOnScreenMsg(instance, NpcStringId.SOUL_REAPER_ARONIA_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ALL_YOUR_SOULS_ARE_MINE);
					break;
				}
				case ODD:
				{
					showOnScreenMsg(instance, NpcStringId.INQUISITOR_ODD_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_WILL_BE_JUDGED_HEATHENS);
					break;
				}
				case EVEN:
				{
					showOnScreenMsg(instance, NpcStringId.JUDGE_EVEN_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_WILL_BE_JUDGED_HEATHENS);
					break;
				}
				case NEMERTESS:
				{
					showOnScreenMsg(instance, NpcStringId.INCARNATION_OF_REVENGE_NEMERTESS, ExShowScreenMessage.TOP_CENTER, 5000, false);
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.REMEMBER_MY_NAME);
					break;
				}
			}
			
			npc.setRandomWalking(false);
		}
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		final Instance instance = npc.getInstanceWorld();
		final Creature mostHated = npc.asAttackable().getMostHated();
		if (isInInstance(instance))
		{
			switch (npc.getId())
			{
				case SIEGE_TANK:
				case MAGIC_CANNON:
				{
					npc.setTarget(mostHated);
					npc.doCast(getRandomEntry(CANNON_GROUP).getSkill());
					break;
				}
				case SAMPSON:
				{
					final boolean sampson50 = instance.getParameters().getBoolean("SAMPSON_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !sampson50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.REINFORCEMENTS_CHARGE);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("SAMPSON_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case HANSON:
				{
					final boolean hanson50 = instance.getParameters().getBoolean("HANSON_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !hanson50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.REINFORCEMENTS_CHARGE);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("HANSON_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case GROM:
				{
					final boolean grom50 = instance.getParameters().getBoolean("GROM_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !grom50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.IT_S_ONLY_THE_BEGINNING);
						npc.abortCast();
						npc.doCast(EMBRYO_BERSERKER.getSkill());
						instance.getParameters().set("GROM_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case MEDVEZ:
				{
					final boolean medvez50 = instance.getParameters().getBoolean("MEDVEZ_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !medvez50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_CAN_NEVER_BE_DEFEATED);
						npc.abortCast();
						npc.doCast(EMBRYO_GUARDIAN.getSkill());
						instance.getParameters().set("MEDVEZ_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case ZIGATAN:
				{
					final boolean zigatan50 = instance.getParameters().getBoolean("ZIGATAN_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !zigatan50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.LORD_ETIS_VAN_ETINA_GRANT_US_UNHOLY_POWER);
						npc.abortCast();
						npc.doCast(EMBRYO_BERSERKER.getSkill());
						instance.getParameters().set("ZIGATAN_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case HUNCHBACK_KWAI:
				{
					final boolean kwai50 = instance.getParameters().getBoolean("KWAI_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !kwai50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.GIVE_UP_YOUR_LIFE_FOR_ETIS_VAN_ETINA);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("KWAI_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case CORNIX:
				{
					final boolean cornix50 = instance.getParameters().getBoolean("CORNIX_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !cornix50)
					{
						for (int i = 0; i < 8; i++)
						{
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.RISE_MY_SLAVES);
							addSpawn(RESURRECTED_CASTLE_ARCHER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("CORNIX_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case CARANIX:
				{
					final boolean caranix50 = instance.getParameters().getBoolean("CARANIX_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !caranix50)
					{
						for (int i = 0; i < 8; i++)
						{
							addSpawn(RESURRECTED_CASTLE_ARCHER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("CARANIX_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case JONADAN:
				{
					final boolean jonadan50 = instance.getParameters().getBoolean("JONADAN_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !jonadan50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.CHARGE_AHEAD_DRIVE_OFF_THE_HEATHENS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("JONADAN_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case DEMIEN:
				{
					final boolean demien50 = instance.getParameters().getBoolean("DEMIEN_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !demien50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.CHARGE_AHEAD_DRIVE_OFF_THE_HEATHENS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("DEMIEN_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case BERK:
				{
					final boolean berk50 = instance.getParameters().getBoolean("BERK_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !berk50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.AARRGGHH_IT_HURTS_I_M_GOING_TO_DESTROY_THEM_ALL);
						npc.abortCast();
						npc.doCast(EMBRYO_GUARDIAN.getSkill());
						instance.getParameters().set("BERK_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case TARKU:
				{
					final boolean tarku50 = instance.getParameters().getBoolean("TARKU_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !tarku50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.TIME_TO_EARN_YOUR_PAY_YOU_SLOBS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(ORC_MERCENARY, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("TARKU_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case TARPIN:
				{
					final boolean tarpin50 = instance.getParameters().getBoolean("TARPIN_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !tarpin50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.TIME_TO_EARN_YOUR_PAY_YOU_SLOBS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(ORC_MERCENARY, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("TARPIN_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case SAFE_VAULT:
				{
					final boolean safe50 = instance.getParameters().getBoolean("SAFE_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !safe50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WARNING_WARNING_DANGER);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("SAFE_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(VAULT_GROUP).getSkill());
					}
					break;
				}
				case SECRET_VAULT:
				{
					final boolean secret50 = instance.getParameters().getBoolean("SECRET_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !secret50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WARNING_WARNING_DANGER);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("SECRET_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(VAULT_GROUP).getSkill());
					}
					break;
				}
				case SAKUM:
				{
					final boolean sakum50 = instance.getParameters().getBoolean("SAKUM_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !sakum50)
					{
						for (int i = 0; i < 8; i++)
						{
							addSpawn(POSLOF, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("SAKUM_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case CRAZY_TYPHOON:
				{
					final boolean typhoon50 = instance.getParameters().getBoolean("TYPHOON_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !typhoon50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HAHAHA_UNLEASH_THE_DOGS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(POSLOF, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("TYPHOON_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case CURSED_HAREN:
				{
					final boolean haren50 = instance.getParameters().getBoolean("HAREN_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !haren50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.IT_S_PAINFUL_MY_BODY_IS_MOVING_AGAINST_MY_WILL_AARGHH);
						npc.abortCast();
						npc.doCast(EMBRYO_GUARDIAN.getSkill());
						instance.getParameters().set("HAREN_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case FLYNT:
				{
					final boolean flynt50 = instance.getParameters().getBoolean("FLYNT_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !flynt50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.AARRGGHHH);
						npc.abortCast();
						npc.doCast(EMBRYO_BERSERKER.getSkill());
						instance.getParameters().set("FLYNT_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case HARP:
				{
					final boolean harp50 = instance.getParameters().getBoolean("HARP_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !harp50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DANGEROUS_ENEMY_DEACTIVATING_LIMITERS);
						npc.abortCast();
						npc.doCast(EMBRYO_BERSERKER.getSkill());
						instance.getParameters().set("HARP_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case MALISS:
				{
					final boolean maliss50 = instance.getParameters().getBoolean("MALISS_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !maliss50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ISADORA_HELP);
						npc.abortCast();
						npc.doCast(EMBRYO_GUARDIAN.getSkill());
						instance.getParameters().set("MALISS_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case ISADORA:
				{
					final boolean isadora50 = instance.getParameters().getBoolean("ISADORA_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !isadora50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_WILL_PAY_FOR_THIS_INSULT);
						npc.abortCast();
						npc.doCast(EMBRYO_BERSERKER.getSkill());
						instance.getParameters().set("ISADORA_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(FIGHTER_GROUP).getSkill());
					}
					break;
				}
				case WHITRA:
				{
					final boolean whitra50 = instance.getParameters().getBoolean("WITHRA_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !whitra50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.GUARD_MESSIAH_CASTLE);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_WIZARD, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("WITHRA_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case BLETRA:
				{
					final boolean bletra50 = instance.getParameters().getBoolean("BLETRA_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !bletra50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.GUARD_MESSIAH_CASTLE);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_WIZARD, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("BLETRA_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case UPGRADED_SIEGE_TANK:
				{
					final boolean tank50 = instance.getParameters().getBoolean("TANK_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !tank50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DANGER_SUMMON_RAIDERS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_RAIDER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("TANK_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case VEGIMA:
				{
					final boolean vegima50 = instance.getParameters().getBoolean("VEGIMA_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !vegima50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.LORD_ETIS_VAN_ETINA_GRANT_US_UNHOLY_POWER);
						npc.abortCast();
						npc.doCast(EMBRYO_GUARDIAN.getSkill());
						instance.getParameters().set("VEGIMA_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case VARONIA:
				{
					final boolean varonia50 = instance.getParameters().getBoolean("VARONIA_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !varonia50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.RISE_MY_SLAVES);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(RESURRECTED_CASTLE_ARCHER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("VARONIA_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case ARONIA:
				{
					final boolean varonia50 = instance.getParameters().getBoolean("VARONIA_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !varonia50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.RISE_MY_SLAVES);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(RESURRECTED_CASTLE_ARCHER, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("VARONIA_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case ODD:
				{
					final boolean odd50 = instance.getParameters().getBoolean("ODD_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !odd50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.PUNISH_THE_HEATHENS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_WIZARD, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("ODD_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case EVEN:
				{
					final boolean even50 = instance.getParameters().getBoolean("EVEN_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !even50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.PUNISH_THE_HEATHENS);
						for (int i = 0; i < 8; i++)
						{
							addSpawn(CASTLE_WIZARD, npc.getLocation(), true, 0, false, instance.getId());
						}
						
						instance.getParameters().set("EVEN_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
				case NEMERTESS:
				{
					final boolean nemertess50 = instance.getParameters().getBoolean("NEMERTESS_50", false);
					if ((npc.getCurrentHp() <= (npc.getMaxHp() * 0.5)) && !nemertess50)
					{
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.IT_S_TIME_TO_PAY_BACK_FOR_WHAT_HAPPENED_IN_THE_LABYRINTH_OF_BELIS);
						npc.abortCast();
						npc.doCast(EMBRYO_BERSERKER.getSkill());
						instance.getParameters().set("NEMERTESS_50", true);
					}
					else
					{
						npc.setTarget(mostHated);
						npc.doCast(getRandomEntry(MAGE_GROUP).getSkill());
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final Instance instance = npc.getInstanceWorld();
		if (isInInstance(instance))
		{
			switch (npc.getId())
			{
				case SIEGE_TANK:
				case MAGIC_CANNON:
				{
					if (!bossSet.isEmpty())
					{
						List<Integer> bossList = new ArrayList<>(bossSet);
						Random random = new Random();
						int randomIndex = random.nextInt(bossList.size());
						
						Integer randomBoss = bossList.get(randomIndex);
						bossSet.remove(randomBoss);
						
						addSpawn(randomBoss, npc.getLocation(), true, 0, false, instance.getId());
					}
					break;
				}
			}
			
			if (ArrayUtil.contains(BOSSES, npc.getId()))
			{
				final int zoneId = killer.getVariables().getInt("CURRENT_ZONE", 0);
				switch (zoneId)
				{
					case DEVIANNE_1:
					{
						DEVIANNE_SAYUNE_1.setEnabled(true);
						instance.spawnGroup("DEVIANNE_1_SAYUNE");
						break;
					}
					case DEVIANNE_2:
					{
						DEVIANNE_SAYUNE_2.setEnabled(true);
						instance.spawnGroup("DEVIANNE_2_SAYUNE");
						break;
					}
					case DEVIANNE_3:
					{
						DEVIANNE_SAYUNE_3.setEnabled(true);
						instance.spawnGroup("DEVIANNE_3_SAYUNE");
						break;
					}
					case ELIKIA_1:
					{
						ELIKIA_SAYUNE_1.setEnabled(true);
						instance.spawnGroup("ELIKIA_1_SAYUNE");
						break;
					}
					case ELIKIA_2:
					{
						ELIKIA_SAYUNE_2.setEnabled(true);
						instance.spawnGroup("ELIKIA_2_SAYUNE");
						break;
					}
					case ELIKIA_3:
					{
						ELIKIA_SAYUNE_3.setEnabled(true);
						instance.spawnGroup("ELIKIA_3_SAYUNE");
						break;
					}
					case SPORCHA_1:
					{
						SPORCHA_SAYUNE_1.setEnabled(true);
						instance.spawnGroup("SPORCHA_1_SAYUNE");
						break;
					}
					case SPORCHA_2:
					{
						SPORCHA_SAYUNE_2.setEnabled(true);
						instance.spawnGroup("SPORCHA_2_SAYUNE");
						break;
					}
					case SPORCHA_3:
					{
						SPORCHA_SAYUNE_3.setEnabled(true);
						instance.spawnGroup("SPORCHA_3_SAYUNE");
						break;
					}
					case LEONA_1:
					{
						LEONA_SAYUNE_1.setEnabled(true);
						instance.spawnGroup("LEONA_1_SAYUNE");
						break;
					}
					case LEONA_2:
					{
						LEONA_SAYUNE_2.setEnabled(true);
						instance.spawnGroup("LEONA_2_SAYUNE");
						break;
					}
					case LEONA_3:
					{
						LEONA_SAYUNE_3.setEnabled(true);
						instance.spawnGroup("LEONA_3_SAYUNE");
						break;
					}
				}
				
				showOnScreenMsg(instance, NpcStringId.THE_SAYUNE_TO_THE_NEXT_AREA_HAS_BEEN_ACTIVATED, ExShowScreenMessage.TOP_CENTER, 7000, false);
				instance.getAliveNpcs(MINIONS).forEach(minion -> minion.deleteMe());
				
				int aliveRaid = instance.getParameters().getInt("ALIVE_RAIDS", 11);
				instance.setParameter("ALIVE_RAIDS", aliveRaid - 1);
				
				if (aliveRaid == 0)
				{
					playMovie(instance, Movie.SI_INZONE_FIN);
					instance.getAliveNpcs(MONSTERS).forEach(monster -> monster.deleteMe());
					instance.finishInstance();
				}
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final Instance instance = npc.getInstanceWorld();
		final String htmltext = null;
		if (isInInstance(instance))
		{
			return npc.getId() + ".html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		final Instance instance = creature.getInstanceWorld();
		if ((instance != null) && creature.isPlayer())
		{
			creature.asPlayer().getVariables().set("CURRENT_ZONE", zone.getId());
		}
	}
	
	public static void main(String[] args)
	{
		new MessiahOuter();
	}
}
