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
package ai.areas.GardenOfSpirits.Dinford;

import org.l2jmobius.gameserver.data.xml.MultisellData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.Id;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.npc.OnNpcMenuSelect;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;

/**
 * Dinford in Blackbird Campsite
 * @author Gigi
 * @date 2018-04-07 - [12:07:12]
 */
public class Dinford extends Script
{
	// NPC
	private static final int DINFORD = 34236;
	
	private Dinford()
	{
		addFirstTalkId(DINFORD);
		addTalkId(DINFORD);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "34236.html":
			case "34236-01.html":
			case "34236-02.html":
			case "34236-03.html":
			case "34236-04.html":
			case "34236-05.html":
			{
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@RegisterEvent(EventType.ON_NPC_MENU_SELECT)
	@RegisterType(ListenerRegisterType.NPC)
	@Id(DINFORD)
	public void onNpcMenuSelect(OnNpcMenuSelect event)
	{
		final Player player = event.getTalker();
		final Npc npc = event.getNpc();
		final int ask = event.getAsk();
		final int reply = event.getReply();
		if (ask == -303)
		{
			if (reply == 2178)
			{
				MultisellData.getInstance().separateAndSend(2178, player, npc, false);
			}
			else if (reply == 2179)
			{
				MultisellData.getInstance().separateAndSend(2179, player, npc, false);
			}
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		if (getRandom(10) < 5)
		{
			player.sendPacket(new PlaySound(3, "Npcdialog1.dinfod_faction_1", 0, 0, 0, 0, 0));
		}
		else
		{
			player.sendPacket(new PlaySound(3, "Npcdialog1.dinfod_faction_2", 0, 0, 0, 0, 0));
		}
		
		return "34236.html";
	}
	
	public static void main(String[] args)
	{
		new Dinford();
	}
}
