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
package ai.others.BalthusKnights.Helper.TartiFriend;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.managers.ScriptManager;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureDamageReceived;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;

import instances.BalthusKnights.AntharasNest;
import quests.Q10555_ChargeAtAntharas.Q10555_ChargeAtAntharas;

/**
 * Tarti Friend AI
 * @author Kazumi
 */
public final class TartiFriend extends Script
{
	// NPCs
	private static final int TARTI_FRIEND = 34365;
	
	// Monster
	// private static final int HATCHLING = 24089;
	private static final int GEM_DRAGON_ANTHARAS = 24091;
	private static final int ANTHARAS = 24087;
	private static final int ANTHARAS_TRANSFORM = 24088;
	
	// Skills
	private static final SkillHolder HydroAttackSkill = new SkillHolder(32131, 1);
	// private static final SkillHolder AirRushSkill = new SkillHolder(32132, 1);
	// Misc
	private static final int p_CheckInterval = 3000;
	private static final int p_CheckFirstAntharasInterval = 18000;
	private static final int p_CheckAntharasInterval = 3000;
	private static final int p_CheckFirstTransInterval = 13000;
	private static final int p_CheckTransInterval = 3000;
	private static final int p_TalkInterval = 15000;
	private static boolean _firstAttacked;
	private static boolean _firstAntharasTalk;
	
	public TartiFriend()
	{
		addFirstTalkId(TARTI_FRIEND);
		addSpawnId(TARTI_FRIEND, ANTHARAS, ANTHARAS_TRANSFORM);
		addKillId(GEM_DRAGON_ANTHARAS);
	}
	
	@Override
	public final String onFirstTalk(Npc npc, Player player)
	{
		return "tarti_friend_tu001.htm";
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		
		switch (npc.getId())
		{
			case TARTI_FRIEND:
			{
				ThreadPool.schedule(() ->
				{
					final StatSet npcVars = npc.getVariables();
					npc.setInvul(true);
					_firstAttacked = true;
					if (instance.getTemplateId() == 271)
					{
						_firstAntharasTalk = true;
					}
					
					getTimers().addTimer("p_CheckTimer", p_CheckInterval, npc, null);
					if ((instance.getTemplateId() == 269) || (instance.getTemplateId() == 270))
					{
						getTimers().addTimer("p_TalkTimer", p_TalkInterval, npc, null);
					}
					
					final Player player = instance.getFirstPlayer();
					if (player != null)
					{
						npcVars.set("PLAYER_OBJECT", player);
					}
					
				}, 3000); // 3 Sec
				break;
			}
			case ANTHARAS:
			{
				final Npc tarti = instance.getNpc(TARTI_FRIEND);
				getTimers().cancelTimer("p_CheckTimer", tarti, null);
				getTimers().addTimer("p_CheckAntharasTimer", p_CheckFirstAntharasInterval, tarti, null);
				break;
			}
			case ANTHARAS_TRANSFORM:
			{
				final Npc tarti = instance.getNpc(TARTI_FRIEND);
				getTimers().cancelTimer("p_CheckAntharasTimer", tarti, null);
				getTimers().addTimer("p_CheckTransTimer", p_CheckFirstTransInterval, tarti, null);
				break;
			}
		}
	}
	
	@Override
	public void onTimerEvent(String event, StatSet params, Npc npc, Player player)
	{
		Instance instance = npc.getInstanceWorld();
		if ((instance != null) && event.equals("p_CheckTimer"))
		{
			synchronized (npc)
			{
				if (!npc.isDead() && !npc.isDecayed())
				{
					getTimers().addTimer("p_CheckTimer", p_CheckInterval, npc, null);
					final StatSet npcVars = npc.getVariables();
					final Player target = npcVars.getObject("PLAYER_OBJECT", Player.class);
					if (target != null)
					{
						final double distance = npc.calculateDistance2D(target);
						if (distance > 200)
						{
							final Location loc = new Location(target.getX(), target.getY(), target.getZ() + 50);
							final Location randLoc = new Location(loc.getX() + getRandom(-10, 50), loc.getY() + getRandom(-100, 100), loc.getZ());
							if (distance > 800)
							{
								npc.teleToLocation(loc);
							}
							else
							{
								npc.setRunning();
							}
							
							addMoveToDesire(npc, randLoc, 23);
						}
					}
				}
			}
		}
		else if ((instance != null) && event.equals("p_TalkTimer"))
		{
			synchronized (npc)
			{
				if (!npc.isDead() && !npc.isDecayed())
				{
					getTimers().addTimer("p_TalkTimer", p_TalkInterval, npc, null);
					switch (instance.getTemplateId())
					{
						case 269:
						{
							switch (Rnd.get(3))
							{
								case 1:
								{
									instance.broadcastPacket(new PlaySound(3, "Npcdialog1.tarti_ep50_battle_1", 0, 0, 0, 0, 0));
									npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WHO_LL_BE_SURPRISED_THE_MOST_IF_WE_BECOME_THE_BALTHUS_KNIGHTS);
									break;
								}
								case 2:
								{
									instance.broadcastPacket(new PlaySound(3, "Npcdialog1.tarti_ep50_battle_2", 0, 0, 0, 0, 0));
									npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.AS_IF_ANYONE_WOULD_BE_SCARED_OF_A_HATCHLING);
									break;
								}
								case 3:
								{
									instance.broadcastPacket(new PlaySound(3, "Npcdialog1.tarti_ep50_battle_3", 0, 0, 0, 0, 0));
									npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.ANTHARAS_IS_MINE_I_WON_T_LET_ANYONE_ELSE_TAKE_HIM_NOT_EVEN_YOU);
									break;
								}
							}
							break;
						}
						case 270:
						{
							switch (Rnd.get(2))
							{
								case 1:
								{
									instance.broadcastPacket(new PlaySound(3, "Npcdialog1.tarti_ep50_battle_4", 0, 0, 0, 0, 0));
									npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.IT_LOOKS_JUST_LIKE_THE_HATCHLING_WE_SAW_BEFORE_DOESN_T_IT);
									break;
								}
								case 2:
								{
									instance.broadcastPacket(new PlaySound(3, "Npcdialog1.tarti_ep50_battle_5", 0, 0, 0, 0, 0));
									npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THEY_WOULDN_T_GO_TO_ANTHARAS_WITHOUT_US_WOULD_THEY);
									break;
								}
							}
							break;
						}
					}
				}
			}
		}
		else if ((instance != null) && event.equals("p_CheckAntharasTimer"))
		{
			synchronized (npc)
			{
				if (!npc.isDead() && !npc.isDecayed())
				{
					getTimers().addTimer("p_CheckAntharasTimer", p_CheckAntharasInterval, npc, null);
					final Npc antharas = instance.getNpc(ANTHARAS);
					if (antharas != null)
					{
						npc.setTarget(antharas);
						npc.setRunning();
						addMoveToDesire(npc, npc.getTarget().getLocation(), 23);
						addSkillCastDesire(npc, antharas, HydroAttackSkill, 10000);
					}
				}
			}
		}
		else if ((instance != null) && event.equals("p_CheckTransTimer"))
		{
			synchronized (npc)
			{
				if (!npc.isDead() && !npc.isDecayed())
				{
					getTimers().addTimer("p_CheckTransTimer", p_CheckTransInterval, npc, null);
					if (_firstAntharasTalk)
					{
						instance.broadcastPacket(new PlaySound(3, "Npcdialog1.tarti_ep50_battle_7", 0, 0, 0, 0, 0));
						npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_DIDN_T_KNOW_THAT_ANTHARAS_COULD_POLYMORPH);
						_firstAntharasTalk = false;
					}
					
					final Npc antharas_trans = instance.getNpc(ANTHARAS_TRANSFORM);
					if (antharas_trans != null)
					{
						npc.setTarget(antharas_trans);
						npc.setRunning();
						addMoveToDesire(npc, npc.getTarget().getLocation(), 23);
						addSkillCastDesire(npc, antharas_trans, HydroAttackSkill, 10000);
					}
				}
			}
		}
	}
	
	public void onCreatureDamageReceived(OnCreatureDamageReceived event)
	{
		final Npc npc = event.getTarget().asNpc();
		final Instance instance = npc.getInstanceWorld();
		final Npc tarti = instance.getNpc(TARTI_FRIEND);
		
		if (event.getAttacker().isPlayer())
		{
			switch (instance.getTemplateId())
			{
				case 269:
				case 270:
				{
					if (tarti != null)
					{
						tarti.setTarget(npc);
						tarti.setRunning();
						addMoveToDesire(tarti, tarti.getTarget().getLocation(), 23);
						addSkillCastDesire(tarti, npc, HydroAttackSkill, 10000);
						break;
					}
					break;
				}
				case 271:
				{
					if ((tarti != null) && (!instance.isStatus(3)))
					{
						tarti.setTarget(npc);
						tarti.setRunning();
						if (tarti.getTarget() != null)
						{
							addMoveToDesire(tarti, tarti.getTarget().getLocation(), 23);
						}
						
						addSkillCastDesire(tarti, npc, HydroAttackSkill, 10000);
						if (_firstAttacked)
						{
							instance.broadcastPacket(new PlaySound(3, "Npcdialog1.tarti_ep50_battle_6", 0, 0, 0, 0, 0));
							npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_LL_PROTECT_YOU);
							_firstAttacked = false;
						}
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
		if (npc.getId() == GEM_DRAGON_ANTHARAS)
		{
			final QuestState qs = killer.getQuestState(Q10555_ChargeAtAntharas.class.getSimpleName());
			if ((qs != null) && qs.isCond(2))
			{
				final Quest instance = ScriptManager.getInstance().getScript(AntharasNest.class.getSimpleName());
				if ((instance != null) && (!_firstAttacked))
				{
					_firstAttacked = true;
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new TartiFriend();
	}
}
