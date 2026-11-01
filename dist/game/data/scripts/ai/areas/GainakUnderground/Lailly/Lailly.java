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
package ai.areas.GainakUnderground.Lailly;

import org.l2jmobius.gameserver.managers.InstanceManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;
import org.l2jmobius.gameserver.network.serverpackets.NpcSay;

/**
 * Lailly AI.
 * @author Stayway
 */
public class Lailly extends Script
{
	// NPCs
	private static final int LAILLY = 34181;
	
	// Instances
	private static final int INSTANCE_TAUTI = 261;
	private static final int INSTANCE_KELBIM = 262;
	private static final int INSTANCE_FREYA = 263;
	
	private Lailly()
	{
		addSpawnId(LAILLY);
		addFirstTalkId(LAILLY);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "34181.html":
			{
				htmltext = event;
				break;
			}
			case "spam_text":
			{
				npc.broadcastPacket(new NpcSay(npc.getObjectId(), ChatType.NPC_GENERAL, npc.getId(), NpcStringId.READY_TO_LISTEN_TO_A_STORY_COME_NOW));
				break;
			}
			case "okay":
			{
				final Instance instance = InstanceManager.getInstance().getPlayerInstance(player, false);
				if ((instance != null) && (instance.getEndTime() > System.currentTimeMillis()))
				{
					switch (instance.getTemplateId())
					{
						case INSTANCE_TAUTI:
						case INSTANCE_KELBIM:
						case INSTANCE_FREYA:
						{
							player.teleToLocation(instance.getEnterLocation(), instance);
							break;
						}
					}
					break;
				}
				
				htmltext = "34181-1.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		startQuestTimer("spam_text", 180000, npc, null, true);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "34181.html";
	}
	
	public static void main(String[] args)
	{
		new Lailly();
	}
}
