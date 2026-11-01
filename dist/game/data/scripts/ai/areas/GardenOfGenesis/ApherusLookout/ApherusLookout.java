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
package ai.areas.GardenOfGenesis.ApherusLookout;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * @author Gigi
 */
public class ApherusLookout extends Script
{
	private static final int APHERUS_LOOKOUT = 22964;
	private static final int APHERUS_PACKAGE = 19001;
	private static final int APHERUS_PACKAGE1 = 19002;
	private static final int APHERUS_PACKAGE2 = 19003;
	private static final int APERUS_KEY = 17373;
	
	public ApherusLookout()
	{
		addKillId(APHERUS_LOOKOUT);
		addFirstTalkId(APHERUS_PACKAGE, APHERUS_PACKAGE1, APHERUS_PACKAGE2);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = null;
		if (event.equals("open_bag"))
		{
			if (getRandom(100) < 7)
			{
				npc.broadcastPacket(new ExShowScreenMessage(NpcStringId.MOVED_TO_APHERUS_DIMENSION, ExShowScreenMessage.TOP_CENTER, 3000, true));
				giveItems(player, APERUS_KEY, 1);
			}
			
			npc.deleteMe();
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isPet)
	{
		final Npc aPackage = addSpawn(APHERUS_PACKAGE, npc.getX(), npc.getY(), npc.getZ(), 0, true, 120000, false);
		aPackage.setImmobilized(true);
		final Npc bPackage = addSpawn(APHERUS_PACKAGE1, npc.getX(), npc.getY(), npc.getZ(), 0, true, 120000, false);
		bPackage.setImmobilized(true);
		final Npc cPackage = addSpawn(APHERUS_PACKAGE2, npc.getX(), npc.getY(), npc.getZ(), 0, true, 120000, false);
		cPackage.setImmobilized(true);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "19001.html";
	}
	
	public static void main(String[] args)
	{
		new ApherusLookout();
	}
}
