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
package ai.others.BalthusKnights.Helper.AeoreEvasSaintElena;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.model.Location;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.player.PlayerClass;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;

/**
 * Aeore Eva's Saint Elena AI
 * @author Kazumi
 */
public final class AeoreEvasSaintElena extends Script
{
	// NPCs
	private static final int ELENA = 34379;
	private static final int INVISIBLE_NPC = 18918;
	
	// Skills
	private static final SkillHolder SalvationSkill = new SkillHolder(32181, 1);
	private static final SkillHolder BrilliantHealSkill = new SkillHolder(32150, 1);
	private static final SkillHolder BrilliantRechargeSkill = new SkillHolder(32151, 1);
	
	// Misc
	private static final int p_CheckInterval = 1000;
	private static final int p_TalkInterval = 15000;
	private static final int p_HpCheckInterval = 1000;
	private static final int p_FirstHPCheckInterval = 4000;
	
	public AeoreEvasSaintElena()
	{
		addSpawnId(ELENA, INVISIBLE_NPC);
		addFirstTalkId(ELENA);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		final StatSet npcVars = npc.getVariables();
		
		switch (npc.getId())
		{
			case ELENA:
			{
				if ((instance != null) && (instance.getTemplateId() == 271))
				{
					npc.setInvul(true);
					npc.setTalkable(false);
					npc.setRandomWalking(false);
					npc.setRunning();
					ThreadPool.schedule(new CheckTask(npc, instance), p_CheckInterval);
					ThreadPool.schedule(new HpCheckTask(npc, instance), (p_HpCheckInterval + p_FirstHPCheckInterval));
					
					final Player player = instance.getFirstPlayer();
					if (player != null)
					{
						if (player.getPlayerClass() == PlayerClass.AEORE_HEALER)
						{
							ThreadPool.schedule(new TalkTask(npc, instance), p_TalkInterval);
						}
						
						npcVars.set("PLAYER_OBJECT", player);
						ThreadPool.schedule(() ->
						{
							npc.setTarget(player);
							addSkillCastDesire(npc, npc.getTarget(), SalvationSkill, 100000);
							// npc.say(1717915); // Everyone's life depends on the decisions you make in battle.
						}, 2000L); // 2 sec
					}
				}
				break;
			}
			case INVISIBLE_NPC:
			{
				final Npc elena = instance.getNpc(ELENA);
				if (elena != null)
				{
					final Player player = instance.getFirstPlayer();
					if (player.getPlayerClass() == PlayerClass.AEORE_HEALER)
					{
						elena.setWalking();
						elena.setTalkable(true);
						elena.teleToLocation(171771, 190975, -11536, 25797, instance);
						// TODO: Check how NPC sit down
					}
					else
					{
						ThreadPool.schedule(() ->
						{
							elena.decayMe();
						}, 5000L); // 5 sec
					}
				}
				break;
			}
		}
	}
	
	@Override
	public final String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = null;
		htmltext = "balthus_eolh001.htm";
		return htmltext;
	}
	
	private class CheckTask implements Runnable
	{
		private final Npc _npc;
		private final Instance _instance;
		
		public CheckTask(Npc npc, Instance instance)
		{
			_npc = npc;
			_instance = instance;
		}
		
		@Override
		public void run()
		{
			if ((_instance != null) && (_instance.getStatus() < 4))
			{
				if ((_npc != null) && !_npc.isDead() && !_npc.isDecayed())
				{
					ThreadPool.schedule(new CheckTask(_npc, _instance), p_CheckInterval);
					final StatSet npcVars = _npc.getVariables();
					final Player c_ai0 = npcVars.getObject("PLAYER_OBJECT", Player.class);
					if (c_ai0 != null)
					{
						final double distance = _npc.calculateDistance2D(c_ai0);
						if (distance > 300)
						{
							final Location loc = new Location(c_ai0.getX(), c_ai0.getY(), c_ai0.getZ() + 50);
							final Location randLoc = new Location(loc.getX() + getRandom(-100, 100), loc.getY() + getRandom(-100, 100), loc.getZ());
							if (distance > 600)
							{
								_npc.teleToLocation(loc);
							}
							else
							{
								_npc.setRunning();
							}
							
							addMoveToDesire(_npc, randLoc, 23);
						}
					}
				}
			}
		}
	}
	
	private class HpCheckTask implements Runnable
	{
		private final Npc _npc;
		private final Instance _instance;
		
		public HpCheckTask(Npc npc, Instance instance)
		{
			_npc = npc;
			_instance = instance;
		}
		
		@Override
		public void run()
		{
			if ((_instance != null) && (_instance.getStatus() < 4))
			{
				if ((_npc != null) && !_npc.isDead() && !_npc.isDecayed())
				{
					ThreadPool.schedule(new HpCheckTask(_npc, _instance), p_HpCheckInterval);
					final StatSet npcVars = _npc.getVariables();
					final Player c_ai0 = npcVars.getObject("PLAYER_OBJECT", Player.class);
					if ((c_ai0 != null) && !c_ai0.isDead() && (c_ai0.getCurrentHpPercent() < 80))
					{
						_npc.setTarget(c_ai0);
						addSkillCastDesire(_npc, _npc.getTarget(), BrilliantHealSkill, 100000);
					}
					else if ((c_ai0 != null) && !c_ai0.isDead() && (c_ai0.getCurrentMpPercent() < 40))
					{
						_npc.setTarget(c_ai0);
						addSkillCastDesire(_npc, _npc.getTarget(), BrilliantRechargeSkill, 100000);
					}
				}
			}
		}
	}
	
	private class TalkTask implements Runnable
	{
		private final Npc _npc;
		private final Instance _instance;
		
		public TalkTask(Npc npc, Instance instance)
		{
			_npc = npc;
			_instance = instance;
		}
		
		@Override
		public void run()
		{
			
			if ((_instance != null) && (_instance.getStatus() < 4))
			{
				if ((_npc != null) && !_npc.isDead() && !_npc.isDecayed())
				{
					ThreadPool.schedule(new TalkTask(_npc, _instance), p_TalkInterval);
					switch (Rnd.get(1, 4))
					{
						case 1:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.elena_ep50_battle_1", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WE_VE_GOT_TO_STAY_ALIVE_UNTIL_THE_VERY_END);
							break;
						}
						case 2:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.elena_ep50_battle_2", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.HEY_YOU_WON_T_BE_HEALED_IF_YOU_STRAY_TOO_FAR_AWAY);
							break;
						}
						case 3:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.elena_ep50_battle_3", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.THE_CHOICE_YOU_MAKE_AT_THE_MOST_CRITICAL_MOMENT_WILL_DECIDE_EVERYONE_S_FATE);
							break;
						}
						case 4:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.elena_ep50_battle_4", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DARN_IT_WHAT_IS_THAT_NOW_I_WASN_T_TOLD_ABOUT_THIS);
							break;
						}
					}
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new AeoreEvasSaintElena();
	}
}
