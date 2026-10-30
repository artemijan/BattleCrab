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
package ai.others.BalthusKnights.Helper.EvisceratorTariah;

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
 * Eviscerator Tariah AI
 * @author Kazumi
 */
public final class EvisceratorTariah extends Script
{
	// NPCs
	private static final int TARIAH = 34377;
	
	// Monsters
	private static final int ANTHARAS = 24087;
	private static final int INVISIBLE_NPC = 18918;
	
	// Skills
	private static final SkillHolder GravityHitSkill = new SkillHolder(32155, 1);
	private static final SkillHolder SteelMindSkill = new SkillHolder(32156, 1);
	
	// Misc
	private static final int p_CheckInterval = 3000;
	private static final int p_TalkInterval = 15000;
	
	public EvisceratorTariah()
	{
		addSpawnId(TARIAH, INVISIBLE_NPC);
		addFirstTalkId(TARIAH);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		
		switch (npc.getId())
		{
			case TARIAH:
			{
				if ((instance != null) && (instance.getTemplateId() == 271))
				{
					npc.setInvul(true);
					npc.setTalkable(false);
					final Npc antharas = instance.getNpc(ANTHARAS);
					ThreadPool.schedule(new CheckTask(npc, instance), p_CheckInterval);
					if (instance.getFirstPlayer().getPlayerClass() == PlayerClass.EVISCERATOR_BALTHUS)
					{
						ThreadPool.schedule(new TalkTask(npc, instance), p_TalkInterval);
					}
					
					ThreadPool.schedule(() ->
					{
						npc.setRandomWalking(false);
						npc.setTarget(antharas);
						npc.setRunning();
						addAttackDesire(npc, antharas);
					}, 7000L); // 7 sec
				}
				break;
			}
			case INVISIBLE_NPC:
			{
				final Npc tariah = instance.getNpc(TARIAH);
				if (tariah != null)
				{
					final Player player = instance.getFirstPlayer();
					if (player.getPlayerClass() == PlayerClass.EVISCERATOR_BALTHUS)
					{
						tariah.setWalking();
						tariah.setTalkable(true);
						tariah.teleToLocation(171771, 190975, -11536, 25797, instance);
						tariah.setTarget(tariah);
					}
					else
					{
						ThreadPool.schedule(() ->
						{
							tariah.decayMe();
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
						if (Rnd.get(1000) > 333)
						{
							final Npc antharas = _instance.getNpc(ANTHARAS);
							_npc.setTarget(antharas);
							addSkillCastDesire(_npc, _npc.getTarget(), GravityHitSkill, 20000);
						}
						else
						{
							_npc.setTarget(_npc);
							addSkillCastDesire(_npc, _npc.getTarget(), SteelMindSkill, 20000);
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
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.taria_ep50_battle_1", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_WONDER_HOW_OLD_THIS_DRAGON_IS);
							break;
						}
						case 2:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.taria_ep50_battle_2", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.I_M_FAST_HUH_I_CAN_ATTACK_EVEN_FASTER);
							break;
						}
						case 3:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.taria_ep50_battle_3", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_HAVE_TO_BE_ABLE_TO_USE_YOUR_POWER_AT_THE_RIGHT_TIME);
							break;
						}
						case 4:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.taria_ep50_battle_4", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WOW_I_D_LOVE_TO_DO_SOMETHING_LIKE_THAT_TOO);
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
		htmltext = "balthus_lenker001.htm";
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new EvisceratorTariah();
	}
}
