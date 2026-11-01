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
package ai.areas.FaeronVillage.AltarOfSouls;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Altar of Souls AI.
 * @author Mobius
 */
public class AltarOfSouls extends Script
{
	// NPCs
	private static final int ALTAR_OF_SOULS = 33920;
	private static final int LADAR = 25942;
	private static final int CASSIUS = 25943;
	private static final int TERAKAN = 25944;
	
	// Items
	private static final int APPARITION_STONE_88 = 38572;
	private static final int APPARITION_STONE_93 = 38573;
	private static final int APPARITION_STONE_98 = 38574;
	
	// Misc
	private Npc BOSS_88;
	private Npc BOSS_93;
	private Npc BOSS_98;
	
	private AltarOfSouls()
	{
		addStartNpc(ALTAR_OF_SOULS);
		addFirstTalkId(ALTAR_OF_SOULS);
		addTalkId(ALTAR_OF_SOULS);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "request_boss_88":
			{
				if ((BOSS_88 != null) && !BOSS_88.isDead())
				{
					return "33920-4.html";
				}
				
				if (hasQuestItems(player, APPARITION_STONE_88))
				{
					takeItems(player, APPARITION_STONE_88, 1);
					BOSS_88 = addSpawn(TERAKAN, player.getX() + getRandom(-300, 300), player.getY() + getRandom(-300, 300), player.getZ() + 20, getRandom(64000), false, 0, true);
					return "33920-1.html";
				}
				
				return "33920-7.html";
			}
			case "request_boss_93":
			{
				if ((BOSS_93 != null) && !BOSS_93.isDead())
				{
					return "33920-5.html";
				}
				
				if (hasQuestItems(player, APPARITION_STONE_93))
				{
					takeItems(player, APPARITION_STONE_93, 1);
					BOSS_93 = addSpawn(CASSIUS, player.getX() + getRandom(-300, 300), player.getY() + getRandom(-300, 300), player.getZ() + 20, getRandom(64000), false, 0, true);
					return "33920-2.html";
				}
				
				return "33920-8.html";
			}
			case "request_boss_98":
			{
				if ((BOSS_98 != null) && !BOSS_98.isDead())
				{
					return "33920-6.html";
				}
				
				if (hasQuestItems(player, APPARITION_STONE_98))
				{
					takeItems(player, APPARITION_STONE_98, 1);
					BOSS_98 = addSpawn(LADAR, player.getX() + getRandom(-300, 300), player.getY() + getRandom(-300, 300), player.getZ() + 20, getRandom(64000), false, 0, true);
					return "33920-3.html";
				}
				
				return "33920-9.html";
			}
		}
		
		return super.onEvent(event, npc, player);
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "33920.html";
	}
	
	public static void main(String[] args)
	{
		new AltarOfSouls();
	}
}
