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
package ai.others.BalthusKnights.Npcs.Tarti;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.variables.PlayerVariables;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.ServerPacket;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowHtml;

/**
 * Tarti AI
 * @author Kazumi
 */
public final class Tarti extends Script
{
	// NPCs
	private static final int TARTI = 34359;
	
	// Misc
	private static final int[] SLOTS =
	{
		Inventory.PAPERDOLL_HEAD,
		Inventory.PAPERDOLL_GLOVES,
		Inventory.PAPERDOLL_CHEST,
		Inventory.PAPERDOLL_LEGS,
		Inventory.PAPERDOLL_FEET,
		Inventory.PAPERDOLL_RHAND,
		Inventory.PAPERDOLL_LFINGER,
		Inventory.PAPERDOLL_RFINGER,
		Inventory.PAPERDOLL_LEAR,
		Inventory.PAPERDOLL_REAR,
		Inventory.PAPERDOLL_NECK
	};
	
	public Tarti()
	{
		addFirstTalkId(TARTI);
		addAggroRangeEnterId(TARTI);
	}
	
	@Override
	public final String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = null;
		
		if (player.getVariables().getInt(PlayerVariables.BALTHUS_PHASE, 1) < 3)
		{
			for (int slot : SLOTS)
			{
				if (!player.getInventory().isPaperdollSlotEmpty(slot))
				{
					switch (player.getPlayerClass())
					{
						case SAYHA_SEER_BALTHUS:
						{
							showOnScreenMsg(player, NpcStringId.PRESS_ALT_K_TO_OPEN_THE_SKILL_WINDOW_NAND_ADD_HYDRO_ATTACK_IN_THE_ACTIVE_TAB_TO_THE_SHORTCUTS, ExShowScreenMessage.TOP_CENTER, 30000, false);
							break;
						}
						case FEOH_WIZARD:
						{
							showOnScreenMsg(player, NpcStringId.PRESS_ALT_K_TO_OPEN_THE_SKILL_WINDOW_NAND_ADD_ELEMENTAL_SPIKE_IN_THE_ACTIVE_TAB_TO_THE_SHORTCUTS, ExShowScreenMessage.TOP_CENTER, 30000, false);
							break;
						}
						case AEORE_HEALER:
						{
							showOnScreenMsg(player, NpcStringId.PRESS_ALT_K_TO_OPEN_THE_SKILL_WINDOW_NAND_ADD_DARK_BLAST_IN_THE_ACTIVE_TAB_TO_THE_SHORTCUTS, ExShowScreenMessage.TOP_CENTER, 30000, false);
							break;
						}
						default:
						{
							showOnScreenMsg(player, NpcStringId.PRESS_ALT_K_TO_OPEN_THE_SKILL_WINDOW_NYOU_CAN_ADD_THE_SKILLS_IN_THE_ACTIVE_TAB_TO_THE_SHORTCUTS, ExShowScreenMessage.TOP_CENTER, 30000, false);
							break;
						}
					}
					
					final ServerPacket packet = new ExTutorialShowId(9);
					player.sendPacket(packet);
					htmltext = "tarti_balthus001.htm";
				}
				else
				{
					final ServerPacket packet = new ExTutorialShowId(16);
					player.sendPacket(packet);
					showOnScreenMsg(player, NpcStringId.DOUBLE_CLICK_PAULINA_S_EQUIPMENT_SET_TO_OBTAIN_THE_EQUIPMENT, ExShowScreenMessage.TOP_CENTER, 30000, false);
					htmltext = "tarti_balthus007.htm";
				}
			}
		}
		else
		{
			htmltext = "tarti_balthus002.htm";
		}
		
		return htmltext;
	}
	
	@Override
	public void onAggroRangeEnter(Npc npc, Player player, boolean isSummon)
	{
		if (!player.isDead())
		{
			// TODO: Move to EnterWorld
			final ServerPacket packet = new ExTutorialShowId(5);
			player.sendPacket(packet);
			
			player.sendPacket(new TutorialShowHtml("popup.htm"));
			
			// TODO:
			// On Tutorial click:
			// final ServerPacket packet = new ExTutorialShowId(HelpPageType.CHAT);
			// player.sendPacket(packet);
			player.getVariables().set(PlayerVariables.BALTHUS_PHASE, 1);
			showOnScreenMsg(player, NpcStringId.TARTI_IS_WORRIED_ABOUT_S1, ExShowScreenMessage.TOP_CENTER, 300000, false, player.getName());
		}
	}
	
	public static void main(String[] args)
	{
		new Tarti();
	}
}
