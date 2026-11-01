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
package ai.others.BalthusKnights.Helper.OthellFortuneSeekerMion;

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
 * Othell Fortune Seeker Mion AI
 * @author Kazumi
 */
public final class OthellFortuneSeekerMion extends Script
{
	// NPCs
	private static final int MION = 34375;
	
	// Monsters
	private static final int ANTHARAS = 24087;
	private static final int INVISIBLE_NPC = 18918;
	
	// Skills
	private static final SkillHolder ShadowIllusionSkill = new SkillHolder(32141, 1);
	private static final SkillHolder AngelOfDeathSkill = new SkillHolder(32142, 1);
	
	// Misc
	private static final int p_CheckInterval = 3000;
	private static final int p_TalkInterval = 15000;
	
	public OthellFortuneSeekerMion()
	{
		addSpawnId(MION, INVISIBLE_NPC);
		addFirstTalkId(MION);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		final Instance instance = npc.getInstanceWorld();
		
		switch (npc.getId())
		{
			case MION:
			{
				if ((instance != null) && (instance.getTemplateId() == 271))
				{
					npc.setInvul(true);
					npc.setTalkable(false);
					final Npc antharas = instance.getNpc(ANTHARAS);
					ThreadPool.schedule(new CheckTask(npc, instance), p_CheckInterval);
					if (instance.getFirstPlayer().getPlayerClass() == PlayerClass.OTHELL_ROGUE)
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
				final Npc mion = instance.getNpc(MION);
				if (mion != null)
				{
					final Player player = instance.getFirstPlayer();
					if (player.getPlayerClass() == PlayerClass.OTHELL_ROGUE)
					{
						mion.setWalking();
						mion.setTalkable(true);
						mion.teleToLocation(171771, 190975, -11536, 25797, instance);
						// TODO: Check how NPC sit down
					}
					else
					{
						ThreadPool.schedule(() ->
						{
							mion.decayMe();
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
								addSkillCastDesire(_npc, _npc.getTarget(), ShadowIllusionSkill, 20000);
								break;
							}
							default:
							{
								addSkillCastDesire(_npc, _npc.getTarget(), AngelOfDeathSkill, 20000);
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
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.mion_ep50_battle_1", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.IT_S_CRUCIAL_THAT_YOU_ATTACK_WHERE_THE_ENEMY_IS_MOST_VULNERABLE);
							break;
						}
						case 2:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.mion_ep50_battle_2", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.OUR_SINGLE_POWERFUL_ATTACK_MAY_PRESENT_AN_OPPORTUNITY_TO_FELL_THE_ENEMY);
							break;
						}
						case 3:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.mion_ep50_battle_3", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.SOMETIMES_IT_S_MORE_EFFECTIVE_TO_ATTACK_THE_ENEMY_FROM_BEHIND);
							break;
						}
						case 4:
						{
							_instance.broadcastPacket(new PlaySound(3, "Npcdialog1.mion_ep50_battle_4", 0, 0, 0, 0, 0));
							_npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.WE_CAN_HIDE_AT_THE_MOMENT_OF_DANGER);
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
		htmltext = "balthus_othel001.htm";
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new OthellFortuneSeekerMion();
	}
}
