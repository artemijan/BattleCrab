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
package instances.MysticTavern.StoryOfTauti;

import java.util.Collection;
import java.util.List;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.managers.ZoneManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.FriendlyNpc;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.skill.AbnormalVisualEffect;
import org.l2jmobius.gameserver.model.skill.BuffInfo;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.zone.ZoneType;
import org.l2jmobius.gameserver.model.zone.type.ScriptZone;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExSendUIEvent;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.OnEventTrigger;

import quests.Q00833_DevilsTreasureTauti.Q00833_DevilsTreasureTauti;

/**
 * Mystic Tavern Tauti Instance
 * @VIDEO https://www.youtube.com/watch?v=uPXWZ1ZCtFk
 * @author Gigi, Mobius
 */
public class StoryOfTauti extends InstanceScript
{
	// NPC
	private static final int DETON = 34170;
	private static final int FLAME_FLOWER = 19606;
	private static final int SEAL_DEVICE = 19608;
	
	// Monsters
	private static final int FLAME_STACATO = 23681;
	private static final int FLAME_SCORPION = 23682;
	private static final int FLAME_GOLEM = 23680;
	private static final int FLAME_SCARAB = 23709;
	private static final int SEAL_TOMBSTONE = 19607;
	private static final int SEAL_ARCHANGEL = 23683;
	private static final int SEALED_ANGEL = 23685;
	private static final int NPC_1 = 19626;
	
	// Item
	private static final int FLAME_FLOWER_BUD = 46554;
	
	// Misc
	private static final int TEMPLATE_ID = 261;
	private static final ScriptZone FLAME_FLOWER_ZONE = ZoneManager.getInstance().getZoneById(80027, ScriptZone.class);
	private static final ScriptZone START_ZONE = ZoneManager.getInstance().getZoneById(80028, ScriptZone.class);
	private static final ScriptZone SCORPION_ZONE = ZoneManager.getInstance().getZoneById(80029, ScriptZone.class);
	private static final ScriptZone GOLEM_ZONE = ZoneManager.getInstance().getZoneById(80030, ScriptZone.class);
	private static final ScriptZone TOMBSTONE_ZONE = ZoneManager.getInstance().getZoneById(80031, ScriptZone.class);
	private static final ScriptZone ANGEL_ZONE = ZoneManager.getInstance().getZoneById(80032, ScriptZone.class);
	private static final ScriptZone SCARAB_ZONE = ZoneManager.getInstance().getZoneById(80033, ScriptZone.class);
	private static final SkillHolder DECREASE_PDEF = new SkillHolder(18515, 1);
	private static final SkillHolder SUMMON_FLAME_FLOWER = new SkillHolder(18513, 1);
	private static final SkillHolder SEAL_ARCHANGEL_WRATH = new SkillHolder(16572, 1);
	private static final Location DETON_MOVE = new Location(143641, -149193, -8072);
	private static final Location DETON_MOVE_1 = new Location(151084, -152315, -9072);
	private static final Location DETON_MOVE_2 = new Location(151618, -152823, -9064);
	private static final Location DETON_MOVE_3 = new Location(153296, -145602, -11384);
	private static final Location TELEPORT = new Location(153267, -148441, -11560);
	private static final int ZONE_1ST_TRIGER = 24137770;
	private static final int SKILL_TRIGER = 24138880;
	protected int _count = 0;
	
	public StoryOfTauti()
	{
		super(TEMPLATE_ID);
		addSpawnId(FLAME_FLOWER, FLAME_STACATO, FLAME_SCORPION, FLAME_SCARAB, FLAME_GOLEM, NPC_1, SEAL_ARCHANGEL);
		addAttackId(SEAL_TOMBSTONE, SEAL_ARCHANGEL);
		addKillId(FLAME_STACATO, FLAME_SCORPION, SEAL_TOMBSTONE, SEALED_ANGEL);
		addSkillSeeId(FLAME_FLOWER, SEAL_TOMBSTONE);
		addFirstTalkId(DETON);
		addEnterZoneId(FLAME_FLOWER_ZONE.getId(), START_ZONE.getId(), SCORPION_ZONE.getId(), GOLEM_ZONE.getId(), TOMBSTONE_ZONE.getId(), ANGEL_ZONE.getId(), SCARAB_ZONE.getId());
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final Instance world = npc.getInstanceWorld();
		switch (event)
		{
			case "start_story":
			{
				player.standUp();
				enterInstance(player, null, TEMPLATE_ID);
				final Party party = player.getParty();
				if (party != null)
				{
					final Instance instance = player.getInstanceWorld();
					for (Player member : party.getMembers())
					{
						if (member != player)
						{
							member.standUp();
							member.teleToLocation(player, instance);
							instance.addPlayer(member);
							instance.addAllowed(member);
						}
					}
					
					instance.setReenterTime();
				}
				break;
			}
			case "give_map":
			{
				if ((player.isInParty() && player.getParty().isLeader(player)) || player.isGM())
				{
					if (world.isStatus(1) || world.isStatus(4))
					{
						final Npc deton = world.getNpc(DETON);
						deton.setTarget(player);
						deton.setRunning();
						deton.getAI().setIntention(Intention.FOLLOW, player);
					}
					else
					{
						return "34170-01.html";
					}
				}
				break;
			}
			case "check_status":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ARE_YOU_THE_ONES_WHO_WILL_BE_HELPING_OUT_WELCOME_I_VE_BEEN_WAITING_FOR_YOU);
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1000, pl ->
				{
					if ((pl != null) && ((pl.isInParty() && pl.getParty().isLeader(pl)) || pl.isGM()))
					{
						deton.setTarget(pl);
						deton.setRunning();
						deton.getAI().setIntention(Intention.FOLLOW, pl);
					}
				});
				startQuestTimer("msg_1", 7000, deton, null);
				break;
			}
			case "msg_1":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_HAD_A_HARD_TIME_WORKING_BY_MYSELF_I_M_GLAD_YOU_ARE_HERE_NOW);
				startQuestTimer("msg_2", 10000, deton, null);
				break;
			}
			case "msg_2":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DON_T_WORRY_ABOUT_THE_REWARD_WE_LL_FIND_THAT_TREASURE);
				startQuestTimer("msg_3", 10000, deton, null);
				break;
			}
			case "msg_3":
			{
				world.getNpc(DETON).broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_M_GLAD_POWERFUL_PEOPLE_LIKE_YOU_GUYS_ARE_HELPING_ME);
				break;
			}
			case "msg_4":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_I_M_SURE_I_VE_SEEN_THIS_BEFORE_YES_THAT_MEANS_THE_STAKATOS);
				deton.getAI().setIntention(Intention.MOVE_TO, DETON_MOVE);
				startQuestTimer("msg_5", 7000, deton, null);
				break;
			}
			case "msg_5":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.TO_SAVE_THE_FLAME_FLOWER_YOU_NEED_HEAL_WAIT_RADIANT_HEAL_YES_I_M_SURE);
				startQuestTimer("msg_6", 10000, deton, null);
				break;
			}
			case "msg_6":
			{
				world.getNpc(DETON).broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WELL_THERE_S_NOT_MUCH_DIFFERENCE_RIGHT_OR_IS_THERE);
				break;
			}
			case "spawn_stacato":
			{
				world.spawnGroup("flame_stacato");
				startQuestTimer("spawn_stacato_1", 50000, world.getNpc(DETON), null);
				break;
			}
			case "spawn_stacato_1":
			{
				world.spawnGroup("flame_stacato");
				startQuestTimer("spawn_stacato_2", 50000, world.getNpc(DETON), null);
				break;
			}
			case "spawn_stacato_2":
			{
				world.spawnGroup("flame_stacato");
				world.setStatus(3);
				break;
			}
			case "check_flower":
			{
				if ((world != null) && !npc.isDead())
				{
					for (Npc nearby : World.getInstance().getVisibleObjectsInRange(npc, FriendlyNpc.class, 1000))
					{
						if ((nearby.getId() == FLAME_FLOWER) && npc.isScriptValue(0) && nearby.isScriptValue(0))
						{
							world.getNpc(DETON).setRunning();
							addMoveToDesire(npc, nearby.getLocation(), 23);
							if (npc.calculateDistance3D(nearby) < 100)
							{
								nearby.setCurrentHp(nearby.getCurrentHp() - 10000);
								nearby.setScriptValue(1);
								npc.setCurrentHp(npc.getCurrentHp() + 15000);
								npc.setScriptValue(1);
								startQuestTimer("reset_flower", 8000, nearby, null);
								if (nearby.getCurrentHp() < 1)
								{
									nearby.doDie(npc);
								}
							}
						}
					}
					
					startQuestTimer("check_flower", 3000, npc, null);
				}
				break;
			}
			case "reset_flower":
			{
				npc.setScriptValue(0);
				break;
			}
			case "msg_7":
			{
				world.getNpc(DETON).broadcastSay(ChatType.NPC_GENERAL, NpcStringId.LET_S_HAVE_SOME_FUN);
				break;
			}
			case "spawn_scorpion":
			{
				world.spawnGroup("flame_scorpion");
				world.getNpc(DETON).broadcastSay(ChatType.NPC_GENERAL, NpcStringId.EEK_SAVE_THIS_DWARF_I_M_JUST_AN_ARCHAEOLOGIST_I_LL_GIVE_YOU_EVERYTHING_I_HAVE);
				if (getRandom(10) < 5)
				{
					world.spawnGroup("ifrit");
					world.broadcastPacket(new ExShowScreenMessage(NpcStringId.IFRIT, ExShowScreenMessage.TOP_CENTER, 10000, true));
				}
				break;
			}
			case "attack_player":
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1500, pl ->
				{
					if ((pl != null) && !pl.isDead())
					{
						world.getNpc(DETON).setRunning();
						addMoveToDesire(npc, new Location(pl.getX() + getRandom(-40, 40), pl.getY() + getRandom(-40, 40), pl.getZ()), 23);
						addAttackPlayerDesire(npc, pl);
					}
					
					if (!npc.isDead() && !npc.isInCombat())
					{
						startQuestTimer("attack_player", 5000, npc, pl);
					}
				});
				break;
			}
			case "msg_8":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.OH_YOU_TRULY_ARE_AMAZING_YOU_ACTUALLY_DEFEATED_THOSE_NASTY_GUYS);
				startQuestTimer("msg_9", 8000, deton, null);
				break;
			}
			case "msg_9":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THE_PATH_SPLITS_NOW_WELL_LET_S_GO_WHICH_WAY);
				startQuestTimer("msg_10", 10000, deton, null);
				break;
			}
			case "msg_10":
			{
				world.getNpc(DETON).broadcastSay(ChatType.NPC_GENERAL, NpcStringId.BOTH_LOOK_PRETTY_BAD_BUT_WE_MUST_KEEP_GOING);
				break;
			}
			case "spawn_golem":
			{
				final Npc deton = world.getNpc(DETON);
				world.spawnGroup("golem");
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WAHHH_WHAT_ARE_THESE_MONSTERS_HOW_OLD_DO_YOU_HAVE_BE_TO_GET_THAT_BIG);
				world.broadcastPacket(new ExShowScreenMessage(NpcStringId.LOOK_AT_THAT_FLAME_GOLEM_IT_S_TERRIFYING, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
				startQuestTimer("run_away", 10000, deton, null);
				break;
			}
			case "run_away":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_LL_LEAVE_THOSE_MONSTERS_TO_YOU_I_LL_GO_CHECK_OUT_SOMETHING_OVER_THERE_IT_S_VERY_IMPORTANT);
				deton.setTarget(null);
				deton.stopMove(null);
				deton.getAI().setIntention(Intention.MOVE_TO, DETON_MOVE_1);
				startQuestTimer("delete_daton", 3500, deton, null);
				break;
			}
			case "delete_daton":
			{
				world.despawnGroup("general");
				world.spawnGroup("seal_tmbstone");
				break;
			}
			case "msg_11":
			{
				world.getNpc(DETON).broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YES_THE_SEAL_IS_GONE_WELL_YOU_GO_ON_AHEAD_I_LL_MAKE_SOME_RUBBED_COPIES_FIRST);
				break;
			}
			case "msg_12":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_CAN_T_BELIEVE_YOU_ACTUALLY_FELL_FOR_THAT_I_WAS_JUST_USING_YOU_KAHAHA);
				startQuestTimer("msg_13", 9000, deton, null);
				break;
			}
			case "msg_13":
			{
				world.getNpc(DETON).broadcastSay(ChatType.NPC_GENERAL, NpcStringId.TAUTI_SO_THIS_IS_TAUTI_GIVE_ME_YOUR_POWER_YOU_ARE_MINE_KAHAHAHA);
				break;
			}
			case "spawn_scarab":
			{
				final Npc deton = world.getNpc(DETON);
				world.spawnGroup("flame_scarab");
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WAHHH_THIS_WAY_WAS_DANGEROUS_TOO_SAVE_ME);
				world.broadcastPacket(new ExShowScreenMessage(NpcStringId.THAT_FLAME_SCARAB_IT_S_TERRIFYING, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
				startQuestTimer("run_away_1", 8000, deton, null);
				break;
			}
			case "run_away_1":
			{
				final Npc deton = world.getNpc(DETON);
				deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DO_SOMETHING_ABOUT_THESE_MONSTERS_SHOW_ME_YOUR_STRENGTH_I_LL_BE_WAITING_OVER_THERE);
				deton.setTarget(null);
				deton.stopMove(null);
				deton.getAI().setIntention(Intention.MOVE_TO, DETON_MOVE_1);
				startQuestTimer("delete_daton", 5000, deton, null);
				break;
			}
			case "switch_quest":
			{
				World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1000, pl ->
				{
					if (pl.isInParty())
					{
						final Party party = pl.getParty();
						final List<Player> members = party.getMembers();
						for (Player member : members)
						{
							if (member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
							{
								final QuestState qs = member.getQuestState(Q00833_DevilsTreasureTauti.class.getSimpleName());
								if ((qs != null) && qs.isCond(6))
								{
									final Quest qs833 = ScriptManager.getInstance().getScript(Q00833_DevilsTreasureTauti.class.getSimpleName());
									if (qs833 != null)
									{
										qs.setCond(7, true);
									}
								}
							}
						}
					}
				});
				world.broadcastPacket(new OnEventTrigger(ZONE_1ST_TRIGER, true));
				break;
			}
			case "angel_msg":
			{
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HOW_STUPID_IGNORANCE_IS_A_SIN_I_LL_LET_YOU_GO_THIS_ONE_TIME_GO);
				world.despawnGroup("seal_tmbstone");
				break;
			}
			case "angel_teleport":
			{
				npc.teleToLocation(TELEPORT, world);
				startQuestTimer("angel_triger", 2000, npc, player);
				break;
			}
			case "angel_triger":
			{
				world.broadcastPacket(new OnEventTrigger(ZONE_1ST_TRIGER, false));
				world.spawnGroup("singl_angel");
				startQuestTimer("triger", 3000, npc, player);
				break;
			}
			case "triger":
			{
				world.broadcastPacket(new OnEventTrigger(SKILL_TRIGER, true));
				startQuestTimer("end_triger", 12000, npc, player);
				startQuestTimer("clone_1", 1000, npc, player);
				world.despawnGroup("seal_arngels");
				break;
			}
			case "clone_1":
			{
				world.spawnGroup("clone_1");
				startQuestTimer("clone_2", 500, npc, player);
				break;
			}
			case "clone_2":
			{
				world.spawnGroup("clone_2");
				if (world.getAliveNpcCount(NPC_1) < _count)
				{
					startQuestTimer("clone_3", 500, npc, player);
				}
				break;
			}
			case "clone_3":
			{
				world.spawnGroup("clone_3");
				if (world.getAliveNpcCount(NPC_1) < _count)
				{
					startQuestTimer("clone_4", 500, npc, player);
				}
				break;
			}
			case "clone_4":
			{
				world.spawnGroup("clone_4");
				if (world.getAliveNpcCount(NPC_1) < _count)
				{
					startQuestTimer("clone_5", 500, npc, player);
				}
				break;
			}
			case "clone_5":
			{
				world.spawnGroup("clone_5");
				if (world.getAliveNpcCount(NPC_1) < _count)
				{
					startQuestTimer("clone_6", 500, npc, player);
				}
				break;
			}
			case "clone_6":
			{
				world.spawnGroup("clone_6");
				startQuestTimer("clone_7", 500, npc, player);
				break;
			}
			case "clone_7":
			{
				world.spawnGroup("clone_7");
				break;
			}
			case "clone_player":
			{
				npc.setCloneObjId(player.getObjectId());
				if ((npc.getCloneObjId() == player.getObjectId()) && !player.getEffectList().hasAbnormalVisualEffect(AbnormalVisualEffect.STUN))
				{
					player.teleToLocation(npc.getLocation(), world);
					player.getEffectList().startAbnormalVisualEffect(AbnormalVisualEffect.STUN);
					if ((player.getClan() != null) && (player.getClanCrestId() != 0))
					{
						npc.setClanId(player.getClanId());
					}
					
					npc.broadcastStatusUpdate();
				}
				
				npc.setScriptValue(1);
				startQuestTimer("clear_player", 12000, npc, player);
				break;
			}
			case "end_triger":
			{
				world.broadcastPacket(new OnEventTrigger(SKILL_TRIGER, false));
				break;
			}
			case "clear_player":
			{
				player.getEffectList().stopAbnormalVisualEffect(AbnormalVisualEffect.STUN);
				break;
			}
			case "end_instance":
			{
				_count = 0;
				for (Npc n : world.getAliveNpcs())
				{
					n.deleteMe();
				}
				
				world.finishInstance(0);
				break;
			}
		}
		
		return null;
	}
	
	@Override
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			switch (npc.getId())
			{
				case SEAL_TOMBSTONE:
				{
					final Npc deton = world.getNpc(DETON);
					final Npc device = world.getNpc(SEAL_DEVICE);
					final Npc tombstone = world.getNpc(SEAL_TOMBSTONE);
					if (tombstone.isScriptValue(0))
					{
						deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.SEALED_TABLET_ATTACK_THE_FLAME_FLOWERS_OH_PLANT_THE_FLAME_FLOWERS_AROUND_THE_TABLET_AND_ATTACK_IT_NOW);
						tombstone.setScriptValue(1);
						break;
					}
					
					if (tombstone.isScriptValue(1) && (tombstone.getCurrentHpPercent() < 60))
					{
						device.setDisplayEffect(1);
						deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.OH_YES_LOOK_AT_THAT_THE_SEAL_IS_BREAKING_JUST_A_LITTLE_MORE);
						tombstone.setScriptValue(2);
						break;
					}
					
					if (tombstone.isScriptValue(2) && (tombstone.getCurrentHpPercent() < 40))
					{
						if (getRandom(10) < 5)
						{
							world.spawnGroup("arimanes");
							world.broadcastPacket(new ExShowScreenMessage(NpcStringId.ARIMANES, ExShowScreenMessage.TOP_CENTER, 10000, true));
						}
						
						tombstone.setScriptValue(3);
						break;
					}
					
					if (tombstone.isScriptValue(3) && (tombstone.getCurrentHpPercent() < 20))
					{
						device.setDisplayEffect(2);
						tombstone.setScriptValue(4);
						break;
					}
					break;
				}
				case SEAL_ARCHANGEL:
				{
					final Npc archangel = world.getNpc(SEAL_ARCHANGEL);
					if (archangel == null)
					{
						return;
					}
					
					if (world.isStatus(9) && archangel.isScriptValue(0))
					{
						archangel.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DO_NOT_LUST_AFTER_WHAT_S_SEALED_HERE_IT_IS_NOT_YOURS);
						startQuestTimer("angel_msg", 6000, npc, null);
						startQuestTimer("switch_quest", 1000, npc, attacker);
						archangel.setScriptValue(1);
						break;
					}
					
					if (world.isStatus(9) && (archangel.getCurrentHpPercent() < 50))
					{
						archangel.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.TAUTI_MUST_REMAIN_SEALED_HERE);
						startQuestTimer("angel_teleport", 3000, npc, attacker);
						world.setStatus(10);
						SEAL_ARCHANGEL_WRATH.getSkill().applyEffects(npc, attacker);
						break;
					}
					
					if (world.isStatus(10) && (archangel.getCurrentHpPercent() < 30))
					{
						world.spawnGroup("last_deton");
						final Npc deton = world.getNpc(DETON);
						deton.setRunning();
						deton.getAI().setIntention(Intention.MOVE_TO, DETON_MOVE_3);
						startQuestTimer("msg_12", 6000, deton, null);
						deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.NICE_NICE_I_SEE_THAT_EVERYONE_S_FIGHTING_HARD_FOR_ME);
						world.setStatus(11);
						break;
					}
					
					if (world.isStatus(11) && (archangel.getCurrentHpPercent() < 5))
					{
						archangel.setInvul(true);
						SEAL_ARCHANGEL_WRATH.getSkill().applyEffects(npc, attacker);
						world.setStatus(12);
						World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1000, pl ->
						{
							if (pl.isInParty())
							{
								final Party party = pl.getParty();
								final List<Player> members = party.getMembers();
								for (Player member : members)
								{
									if (member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
									{
										final QuestState qs = member.getQuestState(Q00833_DevilsTreasureTauti.class.getSimpleName());
										if ((qs != null) && qs.isCond(7))
										{
											final Quest qs833 = ScriptManager.getInstance().getScript(Q00833_DevilsTreasureTauti.class.getSimpleName());
											if (qs833 != null)
											{
												qs.setCond(8, true);
											}
										}
									}
								}
							}
						});
						startQuestTimer("end_instance", 52000, world.getNpc(DETON), null);
						world.despawnGroup("last_deton");
						world.despawnGroup("last_archagel");
						playMovie(world.getPlayers(), Movie.EPIC_TAUTI_SCENE);
						break;
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			switch (npc.getId())
			{
				case FLAME_STACATO:
				{
					if (world.isStatus(3) && world.getAliveNpcs(Monster.class).isEmpty())
					{
						final Npc deton = world.getNpc(DETON);
						cancelQuestTimer("end_instance", deton, null);
						world.getPlayers().forEach(temp -> temp.sendPacket(new ExSendUIEvent(temp, true, true, 0, 0, NpcStringId.ELAPSED_TIME)));
						world.setStatus(4);
						World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1000, pl ->
						{
							if ((pl.isInParty() && pl.getParty().isLeader(pl)) || pl.isGM())
							{
								deton.setTarget(pl);
								deton.setRunning();
								deton.getAI().setIntention(Intention.FOLLOW, pl);
							}
						});
						deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ANYWAY_THE_STAKATOS_WILL_NOT_COME_OUT_ANYMORE_WHY_WELL);
						startQuestTimer("msg_7", 7000, deton, null);
					}
					break;
				}
				case FLAME_SCORPION:
				{
					if (world.isStatus(5) && (world.getAliveNpcs(Monster.class).size() < 2))
					{
						world.broadcastPacket(new ExShowScreenMessage(NpcStringId.LEFT_OR_RIGHT_WHICH_WAY, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
						startQuestTimer("msg_8", 3000, world.getNpc(DETON), null);
						world.setStatus(6);
					}
					break;
				}
				case SEAL_TOMBSTONE:
				{
					final Npc device = world.getNpc(SEAL_DEVICE);
					device.setDisplayEffect(3);
					device.doDie(npc);
					final Npc deton = world.getNpc(DETON);
					world.broadcastPacket(new ExShowScreenMessage(NpcStringId.LET_S_GO_DOWN_THIS_WAY_I_LL_BE_RIGHT_BEHIND_YOU, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
					deton.setRunning();
					deton.getAI().setIntention(Intention.MOVE_TO, DETON_MOVE_2);
					startQuestTimer("msg_11", 3000, deton, null);
					break;
				}
				case SEALED_ANGEL:
				{
					world.spawnGroup("last_archagel");
					world.openCloseDoor(world.getTemplateParameters().getInt("1_door"), true);
					break;
				}
			}
		}
	}
	
	@Override
	public void onEnterZone(Creature creature, ZoneType zone)
	{
		final Instance world = creature.getInstanceWorld();
		final Player player = creature.asPlayer();
		if (isInInstance(world))
		{
			switch (zone.getId())
			{
				case 80027:
				{
					if (((player.isInParty() && player.getParty().isLeader(player)) || player.isGM()) && world.isStatus(1))
					{
						final Npc deton = world.getNpc(DETON);
						world.setStatus(2);
						deton.setTarget(null);
						deton.stopMove(null);
						startQuestTimer("msg_4", 7000, deton, null);
						world.broadcastPacket(new ExShowScreenMessage(NpcStringId.IT_S_A_FLAME_FLOWER_THESE_SHOULD_COME_IN_HANDY_LATER_ON, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
						world.getPlayers().forEach(temp -> temp.sendPacket(new ExSendUIEvent(temp, false, false, 180, 0, NpcStringId.ELAPSED_TIME)));
						startQuestTimer("end_instance", 190000, deton, null);
						startQuestTimer("spawn_stacato", 5000, deton, null);
					}
					break;
				}
				case 80028:
				{
					if (world.isStatus(0))
					{
						startQuestTimer("check_status", 21000, world.getNpc(DETON), player);
						world.setStatus(1);
					}
					break;
				}
				case 80029:
				{
					if (world.isStatus(4))
					{
						final Npc deton = world.getNpc(DETON);
						world.broadcastPacket(new ExShowScreenMessage(NpcStringId.A_CROSSROADS_I_DON_T_KNOW_WHICH_WAY_WE_SHOULD_TAKE, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
						deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HMM_WHAT_IS_THIS_I_DON_T_THINK_I_VE_BEEN_AROUND_HERE_BEFORE_THIS_EERIE_FEELING);
						startQuestTimer("spawn_scorpion", 5000, deton, player);
						world.setStatus(5);
					}
					break;
				}
				case 80030:
				{
					if (world.isStatus(6))
					{
						final Npc deton = world.getNpc(DETON);
						deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.OKAY_IT_DOES_LOOK_BETTER_THAN_THE_OTHER_WAY_THE_AIR_FEELS_BETTER_ALREADY);
						startQuestTimer("spawn_golem", 7000, deton, player);
						world.setStatus(7);
					}
					break;
				}
				case 80031:
				{
					if (world.isStatus(7))
					{
						world.broadcastPacket(new ExShowScreenMessage(NpcStringId.A_SEALED_TABLET_HUH, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
						world.setStatus(8);
					}
					break;
				}
				case 80032:
				{
					if (world.isStatus(8))
					{
						world.broadcastPacket(new ExShowScreenMessage(NpcStringId.THERE_S_NO_FUTURE_FOR_THOSE_WHO_CANNOT_WIN_AGAINST_THEMSELVES, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false));
						world.spawnGroup("seal_arngels");
						world.setStatus(9);
					}
					break;
				}
				case 80033:
				{
					if (world.isStatus(6))
					{
						final Npc deton = world.getNpc(DETON);
						deton.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THIS_WAY_FEELS_MUCH_SAFER_GOOD_CHOICE_THE_AIR_FEELS_DIFFERENT);
						startQuestTimer("spawn_scarab", 7000, deton, player);
						world.setStatus(7);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			switch (npc.getId())
			{
				case SEAL_DEVICE:
				{
					world.getNpc(SEAL_DEVICE).setScriptValue(0);
					break;
				}
				case FLAME_FLOWER:
				{
					npc.setCurrentHp(npc.getMaxHp() * 0.20);
					for (Npc tombstone : World.getInstance().getVisibleObjectsInRange(npc, Monster.class, 500))
					{
						if (tombstone.getId() == SEAL_TOMBSTONE)
						{
							npc.setTarget(tombstone);
							tryToEffect(npc, tombstone, DECREASE_PDEF.getSkillId());
						}
					}
					break;
				}
				case SEAL_TOMBSTONE:
				{
					world.getNpc(SEAL_TOMBSTONE).setScriptValue(0);
					break;
				}
				case FLAME_STACATO:
				{
					startQuestTimer("check_flower", 3000, npc, null);
					break;
				}
				case FLAME_SCORPION:
				{
					startQuestTimer("attack_player", 2000, npc, null);
					break;
				}
				case FLAME_GOLEM:
				{
					World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1500, player ->
					{
						if (player.isInParty())
						{
							final Party party = player.getParty();
							final List<Player> members = party.getMembers();
							for (Player member : members)
							{
								if (member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
								{
									final QuestState qs = member.getQuestState(Q00833_DevilsTreasureTauti.class.getSimpleName());
									if ((qs != null) && qs.isCond(2))
									{
										final Quest qs833 = ScriptManager.getInstance().getScript(Q00833_DevilsTreasureTauti.class.getSimpleName());
										if (qs833 != null)
										{
											qs.setCond(3, true);
										}
									}
								}
							}
						}
					});
					break;
				}
				case FLAME_SCARAB:
				{
					startQuestTimer("attack_player", 2000, npc, null);
					World.getInstance().forEachVisibleObjectInRange(npc, Player.class, 1500, player ->
					{
						if (player.isInParty())
						{
							final Party party = player.getParty();
							final List<Player> members = party.getMembers();
							for (Player member : members)
							{
								if (member.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
								{
									final QuestState qs = member.getQuestState(Q00833_DevilsTreasureTauti.class.getSimpleName());
									if ((qs != null) && qs.isCond(2))
									{
										final Quest qs833 = ScriptManager.getInstance().getScript(Q00833_DevilsTreasureTauti.class.getSimpleName());
										if (qs833 != null)
										{
											qs.setCond(4, true);
										}
									}
								}
							}
						}
					});
					break;
				}
				case NPC_1:
				{
					for (Player nearby : World.getInstance().getVisibleObjectsInRange(npc, Player.class, 2000))
					{
						if (world.getNpc(NPC_1).isScriptValue(0) && (nearby != null))
						{
							startQuestTimer("clone_player", 500, npc, nearby);
						}
					}
					break;
				}
				case SEAL_ARCHANGEL:
				{
					if (world.isStatus(10))
					{
						world.getNpc(SEAL_ARCHANGEL).setCurrentHp(npc.getMaxHp() * 0.5);
					}
					break;
				}
			}
		}
	}
	
	@Override
	public void onSkillSee(Npc npc, Player caster, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		final Instance world = npc.getInstanceWorld();
		if (isInInstance(world))
		{
			switch (npc.getId())
			{
				case FLAME_FLOWER:
				{
					if ((world.getStatus() < 5) && (npc.getCurrentHp() == npc.getMaxHp()))
					{
						npc.broadcastPacket(new ExShowScreenMessage(NpcStringId.S1_LOOK_INSIDE_YOUR_BAG_YOU_OBTAINED_A_FLAME_FLOWER, ExShowScreenMessage.BOTTOM_RIGHT, 10000, false, caster.getName()));
						giveItems(caster, FLAME_FLOWER_BUD, 1);
						npc.doDie(npc);
						npc.deleteMe();
					}
					break;
				}
				case SEAL_TOMBSTONE:
				{
					if (skill.getId() == SUMMON_FLAME_FLOWER.getSkillId())
					{
						addSpawn(FLAME_FLOWER, caster, false, 60000, false, world.getId());
					}
					break;
				}
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return npc.getId() + ".html";
	}
	
	private void tryToEffect(Npc npc, Creature character, int diseaseId)
	{
		final BuffInfo info = character.getEffectList().getBuffInfoBySkillId(diseaseId);
		final int skillLevel = (info == null) ? 1 : (info.getSkill().getLevel() < 3) ? info.getSkill().getLevel() + 1 : 3;
		final Skill skill = SkillData.getInstance().getSkill(diseaseId, skillLevel);
		if ((skill != null) && SkillCaster.checkUseConditions(npc, skill))
		{
			npc.doCast(skill);
		}
	}
	
	public static void main(String[] args)
	{
		new StoryOfTauti();
	}
}
