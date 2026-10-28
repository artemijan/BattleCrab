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
package instances.BalthusKnights;

import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.model.variables.PlayerVariables;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

import quests.Q10555_ChargeAtAntharas.Q10555_ChargeAtAntharas;

/**
 * Antharas Nest instance zone.
 * @author Kazumi
 */
public final class AntharasNest extends InstanceScript
{
	// NPCs
	private static final int STIG_MACH_FRIEND = 34366;
	private static final int BALTHUS_KNIGHT = 34372;
	
	// Monsters
	private static final int HATCHLING = 24090;
	private static final int GEM_DRAGON_ANTHARAS = 24091;
	private static final int HATCHLING_BOMBER = 24098;
	private static final int CYCLONE = 24099;
	
	// Skills
	private static final SkillHolder PowerBomberSkill = new SkillHolder(32166, 1);
	
	// Misc
	private static final int TEMPLATE_ID = 271;
	
	public AntharasNest()
	{
		super(TEMPLATE_ID);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (event.equals("enterInstance"))
		{
			final QuestState qs = player.getQuestState(Q10555_ChargeAtAntharas.class.getSimpleName());
			if ((qs != null) && qs.isStarted())
			{
				qs.setCond(1, true);
				player.getVariables().set(PlayerVariables.BALTHUS_PHASE, 2);
				qs.set(Integer.toString(GEM_DRAGON_ANTHARAS), 0);
				enterInstance(player, npc, TEMPLATE_ID);
			}
		}
		else if (event.equals("startGemDragonsAttack"))
		{
			final Instance instance = player.getInstanceWorld();
			final Npc stig = instance.getNpc(STIG_MACH_FRIEND);
			instance.setStatus(1);
			if (stig != null)
			{
				if (getRandomBoolean())
				{
					addSpawn(GEM_DRAGON_ANTHARAS, 171654, 191155, -11536, 0, false, 0, false, instance.getId());
				}
				else
				{
					addSpawn(GEM_DRAGON_ANTHARAS, 171562, 190791, -11536, 0, false, 0, false, instance.getId());
				}
			}
		}
		else if (event.equals("continueGemDragonsAttack"))
		{
			final Instance instance = player.getInstanceWorld();
			final Npc stig = instance.getNpc(STIG_MACH_FRIEND);
			if ((stig != null) && instance.isStatus(1))
			{
				if (getRandomBoolean())
				{
					addSpawn(GEM_DRAGON_ANTHARAS, 171654, 191155, -11536, 0, false, 0, false, instance.getId());
				}
				else
				{
					addSpawn(GEM_DRAGON_ANTHARAS, 171562, 190791, -11536, 0, false, 0, false, instance.getId());
				}
			}
		}
		else if (event.equals("startAntharasProgress"))
		{
			final Instance instance = player.getInstanceWorld();
			final Npc stig = instance.getNpc(STIG_MACH_FRIEND);
			if (stig != null)
			{
				instance.setStatus(2);
				instance.getAliveNpcs(HATCHLING, GEM_DRAGON_ANTHARAS).forEach(dragon -> addSpawn(HATCHLING_BOMBER, dragon.getX(), dragon.getY(), dragon.getZ(), 0, false, 0, false, instance.getId()));
				instance.getAliveNpcs(HATCHLING, GEM_DRAGON_ANTHARAS).forEach(dragon -> addSpawn(CYCLONE, dragon.getX(), dragon.getY(), dragon.getZ(), 0, false, 0, false, instance.getId()));
				instance.getAliveNpcs(HATCHLING, GEM_DRAGON_ANTHARAS).forEach(dragon -> dragon.deleteMe());
				stig.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DON_T_TAKE_US_LIGHTLY);
				getTimers().addTimer("ANTHARAS_INIT_TASK_1", 3000L, it1 ->
				{
					stig.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WHAT_IN_THE_WORLD_ARE_YOU_DOING);
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.doCast(PowerBomberSkill.getSkill()));
					instance.getAliveNpcs(HATCHLING_BOMBER).forEach(bomber -> bomber.decayMe());
					instance.getAliveNpcs(BALTHUS_KNIGHT).forEach(knight -> knight.decayMe());
					getTimers().addTimer("ANTHARAS_INIT_TASK_2", 1000L, it2 ->
					{
						instance.getAliveNpcs(CYCLONE).forEach(cyclone -> cyclone.deleteMe());
						getTimers().addTimer("ANTHARAS_INIT_TASK_3", 7000L, it3 ->
						{
							for (Npc antharas : instance.spawnGroup("balthus_anta_2523_01m1"))
							{
								antharas.setRandomWalking(false);
								antharas.asAttackable().setCanReturnToSpawnPoint(false);
								getTimers().addTimer("ANTHARAS_MOVE_TASK", 4000L, mt1 ->
								{
									final Location loc = instance.getTemplateParameters().getLocation("middlePointRoom");
									antharas.setRunning();
									addMoveToDesire(antharas, loc, 23);
								});
							}
							
							showOnScreenMsg(instance, NpcStringId.ANTHARAS_HAS_APPEARED, ExShowScreenMessage.TOP_CENTER, 10000, false);
							getTimers().addTimer("ANTHARAS_INIT_TASK_4", 4000L, it4 ->
							{
								getTimers().addTimer("ANTHARAS_INIT_TASK_5", 7000L, it5 ->
								{
									instance.setStatus(3);
									instance.spawnGroup("balthus_anta_2523_02m3");
									getTimers().addTimer("ANTHARAS_INIT_TASK_6", 1000L, it6 ->
									{
										stig.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.AT_LAST_ANTHARAS_IS_HERE);
									});
								});
							});
						});
					});
				});
			}
		}
		
		return htmltext;
	}
	
	@Override
	protected void onEnter(Player player, Instance instance, boolean firstEnter)
	{
		showOnScreenMsg(player, NpcStringId.TALK_TO_BALTHUS_KNIGHT_CAPTAIN_STIG_MACH, ExShowScreenMessage.TOP_CENTER, 10000, false);
		super.onEnter(player, instance, firstEnter);
	}
	
	public static void main(String[] args)
	{
		new AntharasNest();
	}
}
