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
package ai.others.BalthusKnights.Helper.YulGhostSentinelKayleen;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.commons.util.Rnd;
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
 * Yul Ghost Sentinel Kayleen AI
 * @author Kazumi
 */
public final class YulGhostSentinelKayleen extends Script
{
	// NPCs
	private static final int KAYLEEN = 34382;
	
	// Monsters
	private static final int ANTHARAS = 24087;
	private static final int INVISIBLE_NPC = 18918;
	
	// Skills
	private static final SkillHolder MultipleArrowSkill = new SkillHolder(32143, 1);
	private static final SkillHolder PhoenixArrowSkill = new SkillHolder(32144, 1);
	
	// Misc
	private static final int p_CheckInterval = 3000;
	private static final int p_TalkInterval = 15000;
	
	public YulGhostSentinelKayleen()
	{
		addSpawnId(KAYLEEN, INVISIBLE_NPC);
		addFirstTalkId(KAYLEEN);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		
		switch (npc.getId())
		{
			case KAYLEEN:
			{
				if ((instance != null) && (instance.getTemplateId() == 271))
				{
					npc.setInvul(true);
					npc.setTalkable(false);
					final Npc antharas = instance.getNpc(ANTHARAS);
					if (instance.getFirstPlayer().getPlayerClass() == PlayerClass.YUL_ARCHER)
					{
						ThreadPool.schedule(new TalkTask(npc, instance), p_TalkInterval);
					}
					
					ThreadPool.schedule(() ->
					{
						ThreadPool.schedule(new CheckTask(npc, instance), p_CheckInterval);
						npc.setRandomWalking(false);
						npc.setTargetable(false);
						npc.setTarget(antharas);
						npc.setRunning();
						addAttackDesire(npc, antharas);
					}, 14000L); // 14 sec
				}
				break;
			}
			case INVISIBLE_NPC:
			{
				final Npc kayleen = instance.getNpc(KAYLEEN);
				if (kayleen != null)
				{
					final Player player = instance.getFirstPlayer();
					if (player.getPlayerClass() == PlayerClass.YUL_ARCHER)
					{
						kayleen.setWalking();
						kayleen.setTalkable(true);
						kayleen.setTargetable(true);
						kayleen.teleToLocation(171771, 190975, -11536, 25797, instance);
						// TODO: Check how NPC sit down
					}
					else
					{
						ThreadPool.schedule(() ->
						{
							kayleen.decayMe();
						}, 5000L); // 5 sec
					}
				}
				break;
			}
		}
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
					if (Rnd.get(1000) <= 333)
					{
						final Npc antharas = _instance.getNpc(ANTHARAS);
						_npc.setTarget(antharas);
						switch (Rnd.get(2))
						{
							case 1:
							{
								addSkillCastDesire(_npc, _npc.getTarget(), MultipleArrowSkill, 20000);
								break;
							}
							default:
							{
								addSkillCastDesire(_npc, _npc.getTarget(), PhoenixArrowSkill, 20000);
								break;
							}
						}
					}
					else
					{
						final Npc antharas = _instance.getNpc(ANTHARAS);
						addAttackDesire(_npc, antharas);
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
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.cairin2_ep50_battle_1", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.OUR_STEADFAST_PRESENCE_IN_THE_BACK_WILL_REASSURE_OUR_COMRADES);
							break;
						}
						case 2:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.cairin2_ep50_battle_2", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.MORE_POWERFUL_ATTACKS_DEALING_CRITICAL_WOUNDS);
							break;
						}
						case 3:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.cairin2_ep50_battle_3", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.DON_T_BE_DISTRACTED_FOCUS_AND_ATTACK);
							break;
						}
						case 4:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.cairin2_ep50_battle_4", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.TRUST_YOUR_COMRADES_AND_DON_T_STOP_ATTACKING);
							break;
						}
					}
				}
			}
		}
	}
	
	@Override
	public final String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = null;
		htmltext = "balthus_yr001.htm";
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new YulGhostSentinelKayleen();
	}
}
