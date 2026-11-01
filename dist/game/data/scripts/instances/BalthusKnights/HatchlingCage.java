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

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.ServerPacket;

import quests.Q10553_WhatMattersMoreThanAbility.Q10553_WhatMattersMoreThanAbility;

/**
 * Hatchling Cage instance zone.
 * @author Kazumi
 */
public final class HatchlingCage extends InstanceScript
{
	// MOBs
	private static final int HATCHLING = 24089;
	private static final int GEM_DRAGON = 24097;
	
	// Misc
	private static final int TEMPLATE_ID = 270;
	
	public HatchlingCage()
	{
		super(TEMPLATE_ID);
		addKillId(HATCHLING, GEM_DRAGON);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (event.equals("enterInstance"))
		{
			final QuestState qs = player.getQuestState(Q10553_WhatMattersMoreThanAbility.class.getSimpleName());
			if ((qs != null) && qs.isStarted())
			{
				enterInstance(player, npc, TEMPLATE_ID);
			}
		}
		
		return htmltext;
	}
	
	@Override
	protected void onEnter(Player player, Instance instance, boolean firstEnter)
	{
		showOnScreenMsg(instance, NpcStringId.ATTACK_A_HATCHLING, ExShowScreenMessage.TOP_CENTER, 30000, false);
		super.onEnter(player, instance, firstEnter);
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Instance instance = player.getInstanceWorld();
		final QuestState qs = player.getQuestState(Q10553_WhatMattersMoreThanAbility.class.getSimpleName());
		
		if ((instance != null) && (instance.getTemplateId() == TEMPLATE_ID))
		{
			if (qs.isCond(2))
			{
				if (instance.getAliveNpcs(HATCHLING).isEmpty())
				{
					final ServerPacket packet = new ExTutorialShowId(15);
					player.sendPacket(packet);
					
					showOnScreenMsg(player, NpcStringId.PRESS_ALT_K_TO_OPEN_THE_SKILL_WINDOW_NYOU_CAN_ADD_THE_SKILLS_IN_THE_ACTIVE_TAB_TO_THE_SHORTCUTS, ExShowScreenMessage.TOP_CENTER, 8000, false);
					ThreadPool.schedule(() ->
					{
						instance.spawnGroup("balthus_cage_2523_01m2");
						showOnScreenMsg(player, NpcStringId.USE_A_SKILL_ON_THE_GEM_DRAGON, ExShowScreenMessage.TOP_CENTER, 5000, false);
					}, 5000); // 5 sec
				}
			}
			else if (qs.isCond(3))
			{
				if (instance.getAliveNpcs(GEM_DRAGON).isEmpty())
				{
					instance.finishInstance(0);
				}
			}
		}
	}
	
	public static void main(String[] args)
	{
		new HatchlingCage();
	}
}
