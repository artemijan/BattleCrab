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
package instances.BalthusKnights.FaeronVillage;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;

import quests.Q10557_NewPowerWindsOfFate.Q10557_NewPowerWindsOfFate;

/**
 * Faeron Village instance zone.
 * @author Kazumi
 */
public final class FaeronVillage extends InstanceScript
{
	// NPCs
	private static final int SERENIA = 34394;
	
	// Misc
	private static final int TEMPLATE_ID = 280;
	
	public FaeronVillage()
	{
		super(TEMPLATE_ID);
		addFirstTalkId(SERENIA);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		final Instance instance = npc.getInstanceWorld();
		if (instance != null)
		{
			switch (player.getPlayerClass())
			{
				case EVISCERATOR_BALTHUS:
				{
					return "serenia002.htm";
				}
				case SAYHA_SEER_BALTHUS:
				{
					return "serenia001.htm";
				}
			}
		}
		
		return null;
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (event.equals("enterInstance"))
		{
			final QuestState qs = player.getQuestState(Q10557_NewPowerWindsOfFate.class.getSimpleName());
			if ((qs != null) && qs.isStarted())
			{
				enterInstance(player, npc, TEMPLATE_ID);
			}
		}
		
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new FaeronVillage();
	}
}
