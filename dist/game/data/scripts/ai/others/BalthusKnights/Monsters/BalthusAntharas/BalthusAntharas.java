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
package ai.others.BalthusKnights.Monsters.BalthusAntharas;

import java.util.Collection;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.enums.Movie;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;
import org.l2jmobius.gameserver.util.ArrayUtil;

import quests.Q10555_ChargeAtAntharas.Q10555_ChargeAtAntharas;

/**
 * Balthus Antharas AI
 * @author Kazumi
 */
public final class BalthusAntharas extends Script
{
	// NPCs
	private static final int BALTHUS_KNIGHT = 34372;
	private static final int INVISIBLE_NPC = 18918;
	private static final int TARTI_FRIEND = 34365;
	private static final int STIG_MACH_FRIEND = 34366;
	private static final int HERPHAH = 34367;
	private static final int FELLOW = 34368;
	private static final int ATELD = 34369;
	private static final int PAULIA = 34370;
	private static final int RASH = 34371;
	protected static final int[] BOMBER_TARGETS =
	{
		34365,
		34366,
		34367,
		34368,
		34369,
		34370,
		34371
	};
	
	// Monsters
	private static final int ANTHARAS = 24087;
	private static final int ANTHARAS_TRANSFORM = 24088;
	private static final int HATCHLING = 24090;
	private static final int GEM_DRAGON_ANTHARAS = 24091;
	private static final int HATCHLING_BOMBER = 24098;
	private static final int CYCLONE = 24099;
	
	// Skills
	private static final SkillHolder PowerBomberSkill = new SkillHolder(32166, 1);
	private static final SkillHolder AntharasBreathSkill = new SkillHolder(32160, 1);
	// private static final SkillHolder AntharasAngerSkill = new SkillHolder(32162, 1);
	// private static final SkillHolder CurseOfAntharasSkill = new SkillHolder(32163, 1);
	// private static final SkillHolder AntharasStunSkill = new SkillHolder(32164, 1);
	
	public BalthusAntharas()
	{
		addSpawnId(ANTHARAS, ANTHARAS_TRANSFORM);
		addSkillSeeId(ANTHARAS_TRANSFORM);
	}
	
	@Override
	public void onSkillSee(Npc npc, Player caster, Skill skill, Collection<WorldObject> targets, boolean isSummon)
	{
		if ((skill != null) && (caster != null) && (skill.getId() == AntharasBreathSkill.getSkillId()) && (npc != null) && !npc.isDead() && !npc.isDecayed())
		{
			final int val = (npc.getMaxHp() / 5);
			npc.reduceCurrentHp(val, npc, null);
			if (npc.getCurrentHpPercent() <= 50)
			{
				final Instance instance = npc.getInstanceWorld();
				showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ANTHARAS_HAS_SUFFERED_CRITICAL_INJURIES, ExShowScreenMessage.TOP_CENTER, 5000);
				getTimers().cancelTimer("p_Wave3Task4", null, instance.getFirstPlayer());
				getTimers().cancelTimer("p_Wave3Task5", null, instance.getFirstPlayer());
				getTimers().cancelTimer("p_Wave3Task6", null, instance.getFirstPlayer());
				getTimers().addTimer("p_EndTask", 5000L, null, instance.getFirstPlayer());
			}
			else
			{
				final Instance instance = npc.getInstanceWorld();
				showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ANTHARAS_HP_HAS_DECREASED, ExShowScreenMessage.TOP_CENTER, 5000);
				getTimers().addTimer("p_BomberTask1", 5000L, null, null);
			}
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		
		if ((instance != null) && (instance.getTemplateId() == 271))
		{
			if (npc.getId() == ANTHARAS)
			{
				final StatSet npcVars = npc.getVariables();
				npcVars.set("i_ai0", 0);
				getTimers().addTimer("p_Wave1Task1", 60000L, null, instance.getFirstPlayer());
			}
			else if (npc.getId() == ANTHARAS_TRANSFORM)
			{
				getTimers().addTimer("p_Wave3Task1", 2000L, null, instance.getFirstPlayer());
			}
		}
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		if (player == null)
		{
			return;
		}
		
		final Instance instance = player.getInstanceWorld();
		if (instance != null)
		{
			switch (event)
			{
				// Wave 1
				case "p_Wave1Task1":
				{
					showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ANTHARAS_SUMMONS_YOUNG_DRAGONS, ExShowScreenMessage.TOP_CENTER, 5000);
					addSpawn(CYCLONE, 171614, 189834, -11532, 25544, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 171846, 190102, -11532, 0, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 171891, 190441, -11532, -11464, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172003, 191171, -11532, 0, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172145, 190735, -11532, 20240, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172277, 189867, -11532, -11928, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172544, 190569, -11532, 11272, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172602, 190208, -11532, 0, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172660, 189542, -11532, -10056, false, 0, false, instance.getId());
					getTimers().addTimer("p_Wave1Task2", 4000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave1Task2":
				{
					instance.getAliveNpcs(CYCLONE).forEach(cyclone -> addSpawn(HATCHLING, cyclone.getX(), cyclone.getY(), cyclone.getZ(), 0, false, 0, false, instance.getId()));
					getTimers().addTimer("p_Wave1Task3", 4000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave1Task3":
				{
					instance.getAliveNpcs(HATCHLING).forEach(hatchling -> addSpawn(BALTHUS_KNIGHT, (hatchling.getX() + 20), (hatchling.getY() + 20), hatchling.getZ(), 0, false, 0, false, instance.getId()));
					instance.getAliveNpcs(CYCLONE).forEach(cyclone -> cyclone.decayMe());
					showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.NEW_REINFORCEMENTS_HAVE_ARRIVED, ExShowScreenMessage.TOP_CENTER, 5000);
					getTimers().addTimer("p_Wave1Task4", 30000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave1Task4":
				{
					instance.getAliveNpcs(HATCHLING).forEach(hatchling -> addSpawn(HATCHLING_BOMBER, hatchling.getX(), hatchling.getY(), hatchling.getZ(), 0, false, 0, false, instance.getId()));
					instance.getAliveNpcs(HATCHLING).forEach(hatchling -> hatchling.decayMe());
					getTimers().addTimer("p_Wave1Task5", 2000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave1Task5":
				{
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.doCast(PowerBomberSkill.getSkill()));
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.decayMe());
					instance.getAliveNpcs(BALTHUS_KNIGHT).forEach(knight -> knight.decayMe());
					getTimers().addTimer("p_Wave2Task1", 10000L, null, instance.getFirstPlayer());
					break;
				}
				// Wave 2
				case "p_Wave2Task1":
				{
					showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ANTHARAS_SUMMONS_YOUNG_DRAGONS, ExShowScreenMessage.TOP_CENTER, 5000);
					addSpawn(CYCLONE, 171614, 189834, -11532, 25544, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 171846, 190102, -11532, 0, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 171891, 190441, -11532, -11464, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172003, 191171, -11532, 0, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172145, 190735, -11532, 20240, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172277, 189867, -11532, -11928, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172544, 190569, -11532, 11272, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172602, 190208, -11532, 0, false, 0, false, instance.getId());
					addSpawn(CYCLONE, 172660, 189542, -11532, -10056, false, 0, false, instance.getId());
					getTimers().addTimer("p_Wave2Task2", 4000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave2Task2":
				{
					instance.getAliveNpcs(CYCLONE).forEach(cyclone -> addSpawn(GEM_DRAGON_ANTHARAS, cyclone.getX(), cyclone.getY(), cyclone.getZ(), 0, false, 0, false, instance.getId()));
					getTimers().addTimer("p_Wave2Task3", 4000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave2Task3":
				{
					instance.getAliveNpcs(GEM_DRAGON_ANTHARAS).forEach(gem_dragon -> addSpawn(BALTHUS_KNIGHT, (gem_dragon.getX() + 35), (gem_dragon.getY() + 35), gem_dragon.getZ(), 0, false, 0, false, instance.getId()));
					instance.getAliveNpcs(CYCLONE).forEach(cyclone -> cyclone.decayMe());
					showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.NEW_REINFORCEMENTS_HAVE_ARRIVED, ExShowScreenMessage.TOP_CENTER, 5000);
					getTimers().addTimer("p_Wave2Task4", 30000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave2Task4":
				{
					instance.getAliveNpcs(GEM_DRAGON_ANTHARAS).forEach(gem_dragon -> addSpawn(HATCHLING_BOMBER, gem_dragon.getX(), gem_dragon.getY(), gem_dragon.getZ(), 0, false, 0, false, instance.getId()));
					instance.getAliveNpcs(GEM_DRAGON_ANTHARAS).forEach(gem_dragon -> gem_dragon.decayMe());
					getTimers().addTimer("p_Wave2Task5", 2000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave2Task5":
				{
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.doCast(PowerBomberSkill.getSkill()));
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.decayMe());
					instance.getAliveNpcs(BALTHUS_KNIGHT).forEach(knight -> knight.decayMe());
					getTimers().addTimer("p_TransTask1", 2000L, null, instance.getFirstPlayer());
					break;
				}
				// Transform Part
				case "p_TransTask1":
				{
					instance.getFirstPlayer().abortCast();
					instance.getFirstPlayer().setTarget(null);
					final Npc antharas = instance.getNpc(ANTHARAS);
					getTimers().cancelTimersOf(antharas);
					if (antharas != null)
					{
						antharas.deleteMe();
					}
					
					addSpawn(INVISIBLE_NPC, 173773, 190240, -11501, 32480, false, 0, false, instance.getId());
					playMovie(instance, Movie.SC_ANTARAS_TRANS);
					getTimers().addTimer("p_TransTask2", 8000L, null, instance.getFirstPlayer());
					instance.setStatus(4);
					break;
				}
				case "p_TransTask2":
				{
					showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ANTHARAS_HAS_POLYMORPHED, ExShowScreenMessage.TOP_CENTER, 5000);
					getTimers().addTimer("p_TransTask3", 5000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_TransTask3":
				{
					addSpawn(ANTHARAS_TRANSFORM, 172504, 190240, -11536, 32480, false, 0, false, instance.getId());
					getTimers().addTimer("p_TransTask4", 1000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_TransTask4":
				{
					showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.INJURED_COMRADES_HAVE_RETREATED_NAND_HERPHAH_HAS_ARRIVED_WITH_NEW_REINFORCEMENTS, ExShowScreenMessage.TOP_CENTER, 5000);
					instance.spawnGroup("balthus_anta_2523_02m4");
					getTimers().addTimer("p_DragonWeaponNoteTask1", 2000L, null, instance.getFirstPlayer());
					break;
				}
				// Dragon Weapon Speak task
				case "p_DragonWeaponNoteTask1":
				{
					final QuestState qs = instance.getFirstPlayer().getQuestState(Q10555_ChargeAtAntharas.class.getSimpleName());
					if ((qs != null) && qs.isStarted())
					{
						switch (instance.getFirstPlayer().getPlayerClass())
						{
							case SIGEL_KNIGHT:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ROIEN_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(4, false);
								break;
							}
							case TYRR_WARRIOR:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.BROME_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(5, false);
								break;
							}
							case OTHELL_ROGUE:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.MION_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(6, false);
								break;
							}
							case YUL_ARCHER:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.KAYLEEN_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(7, false);
								break;
							}
							case WYNN_SUMMONER:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ELIAS_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(8, false);
								break;
							}
							case ISS_ENCHANTER:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.TAKHUN_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(9, false);
								break;
							}
							case AEORE_HEALER:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ELENA_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(10, false);
								break;
							}
							case FEOH_WIZARD:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ULTRIAN_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(11, false);
								break;
							}
							case EVISCERATOR_BALTHUS:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.TARIAH_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(12, false);
								break;
							}
							case SAYHA_SEER_BALTHUS:
							{
								showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.NARITA_IS_CALLING_S1, ExShowScreenMessage.TOP_CENTER, 60000, instance.getFirstPlayer().getName());
								qs.setCond(13, false);
								break;
							}
						}
					}
				}
				// Wave 3
				case "p_Wave3Task1":
				{
					final Npc stig = instance.getNpc(STIG_MACH_FRIEND);
					stig.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HERPHAH_I_DIDN_T_KNOW_YOU_D_COME);
					instance.broadcastPacket(new PlaySound(3, "Npcdialog1.stig_ep50_battle_3", 0, 0, 0, 0, 0));
					getTimers().addTimer("p_Wave3Task2", 5000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave3Task2":
				{
					final Npc herphah = instance.getNpc(HERPHAH);
					herphah.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.SAY_THANKS_WHEN_YOU_FEEL_GRATEFUL);
					instance.broadcastPacket(new PlaySound(3, "Npcdialog1.herphah_ep50_battle_6", 0, 0, 0, 0, 0));
					getTimers().addTimer("p_Wave3Task3", 12000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave3Task3":
				{
					final Npc herphah = instance.getNpc(HERPHAH);
					herphah.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HOW_COME_WE_DIDN_T_KNOW_THAT_ANTHARAS_HAD_THESE_ABILITIES_UNTIL_NOW);
					instance.broadcastPacket(new PlaySound(3, "Npcdialog1.herphah_ep50_battle_4", 0, 0, 0, 0, 0));
					getTimers().addTimer("p_Wave3Task4", 12000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave3Task4":
				{
					final Npc herphah = instance.getNpc(HERPHAH);
					herphah.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_WON_T_LET_YOU_TAKE_MORE_LIVES_OF_MY_COMRADES);
					instance.broadcastPacket(new PlaySound(3, "Npcdialog1.herphah_ep50_battle_5", 0, 0, 0, 0, 0));
					getTimers().addTimer("p_Wave3Task5", 15000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave3Task5":
				{
					final Npc herphah = instance.getNpc(HERPHAH);
					herphah.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.TRUST_THE_WEAPON_IN_YOUR_HAND);
					instance.broadcastPacket(new PlaySound(3, "Npcdialog1.herphah_ep50_battle_2", 0, 0, 0, 0, 0));
					getTimers().addTimer("p_Wave3Task6", 15000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_Wave3Task6":
				{
					final Npc herphah = instance.getNpc(HERPHAH);
					herphah.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.AS_LONG_AS_I_M_HERE_YOU_WILL_SURVIVE);
					instance.broadcastPacket(new PlaySound(3, "Npcdialog1.herphah_ep50_battle_3", 0, 0, 0, 0, 0));
					
					// Repeat to Task 4
					getTimers().addTimer("p_Wave3Task4", 15000L, null, instance.getFirstPlayer());
					break;
				}
				// Hatchling Bomber
				case "p_BomberTask1":
				{
					addSpawn(HATCHLING_BOMBER, 171614, 189834, -11532, 25544, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 171846, 190102, -11532, 0, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 171850, 190812, -11532, 0, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 171891, 190441, -11532, -11464, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172003, 191171, -11532, 0, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172035, 189435, -11532, 13000, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172145, 190735, -11532, 20240, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172276, 190370, -11532, 0, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172277, 189867, -11532, -11928, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172406, 191116, -11532, 0, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172544, 190569, -11532, 11272, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172602, 190208, -11532, 0, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172640, 190889, -11532, -9664, false, 0, false, instance.getId());
					addSpawn(HATCHLING_BOMBER, 172660, 189542, -11532, -10056, false, 0, false, instance.getId());
					getTimers().addTimer("p_BomberTask2", 1000L, null, instance.getFirstPlayer());
					break;
				}
				case "p_BomberTask2":
				{
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> World.getInstance().forEachVisibleObject(bomber, Npc.class, character ->
					{
						if ((character != null) && (!character.isDead()) && (ArrayUtil.contains(BOMBER_TARGETS, character.getId()) || character.isPlayable()))
						{
							bomber.setRunning();
							bomber.asAttackable().addDamageHate(character, 0, 100);
							bomber.getAI().setIntention(Intention.ATTACK, character, null);
						}
					}));
					getTimers().addTimer("p_BomberTask3", 14000L, null, null);
					break;
				}
				case "p_BomberTask3":
				{
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.doCast(PowerBomberSkill.getSkill()));
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.decayMe());
					break;
				}
				// End
				case "p_EndTask":
				{
					final Npc antharas_trans = instance.getNpc(ANTHARAS_TRANSFORM);
					if (antharas_trans != null)
					{
						getTimers().cancelTimersOf(antharas_trans);
						antharas_trans.deleteMe();
					}
					
					getTimers().addTimer("p_CallHerphahTask", 1000L, null, instance.getFirstPlayer());
					getTimers().addTimer("p_CallTartiTask", 1000L, null, instance.getFirstPlayer());
					getTimers().addTimer("p_CallStigTask", 1000L, null, instance.getFirstPlayer());
					getTimers().addTimer("p_CallFellowTask", 1000L, null, instance.getFirstPlayer());
					getTimers().addTimer("p_CallAteldTask", 1000L, null, instance.getFirstPlayer());
					getTimers().addTimer("p_CallPauliaTask", 1000L, null, instance.getFirstPlayer());
					getTimers().addTimer("p_CallRaskTask", 1000L, null, instance.getFirstPlayer());
					showOnScreenMsg(instance.getFirstPlayer(), NpcStringId.ANTHARAS_HAS_DISAPPEARED_INSIDE_HIS_NEST_FOR_RECOVERY, ExShowScreenMessage.TOP_CENTER, 5000, instance.getFirstPlayer().getName());
					break;
				}
				case "p_CallHerphahTask":
				{
					final Npc herphah = instance.getNpc(HERPHAH);
					if (herphah != null)
					{
						getTimers().cancelTimersOf(herphah);
						herphah.abortCast();
						herphah.setTarget(null);
						herphah.getAI().setIntention(Intention.IDLE);
						herphah.setTalkable(true);
						instance.setStatus(5);
						final QuestState qs = instance.getFirstPlayer().getQuestState(Q10555_ChargeAtAntharas.class.getSimpleName());
						if ((qs != null) && qs.isStarted())
						{
							qs.setCond(15, false);
						}
					}
					break;
				}
				case "p_CallTartiTask":
				{
					final Npc tarti = instance.getNpc(TARTI_FRIEND);
					if (tarti != null)
					{
						tarti.deleteMe();
					}
					break;
				}
				case "p_CallStigTask":
				{
					final Npc stig = instance.getNpc(STIG_MACH_FRIEND);
					if (stig != null)
					{
						stig.deleteMe();
					}
					break;
				}
				case "p_CallFellowTask":
				{
					final Npc fellow = instance.getNpc(FELLOW);
					if (fellow != null)
					{
						fellow.deleteMe();
					}
					break;
				}
				case "p_CallAteldTask":
				{
					final Npc ateld = instance.getNpc(ATELD);
					if (ateld != null)
					{
						ateld.deleteMe();
					}
					break;
				}
				case "p_CallPauliaTask":
				{
					final Npc paulia = instance.getNpc(PAULIA);
					if (paulia != null)
					{
						paulia.deleteMe();
					}
					break;
				}
				case "p_CallRaskTask":
				{
					final Npc rash = instance.getNpc(RASH);
					if (rash != null)
					{
						rash.deleteMe();
					}
					break;
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new BalthusAntharas();
	}
}
