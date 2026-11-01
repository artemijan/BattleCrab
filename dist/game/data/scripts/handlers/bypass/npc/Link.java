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
package handlers.bypass.npc;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.cache.HtmCache;
import org.l2jmobius.gameserver.handler.IBypassHandler;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Teleporter;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

public class Link implements IBypassHandler
{
	private static final String[] COMMANDS =
	{
		"Link"
	};
	
	private static final Set<String> VALID_LINKS = new HashSet<>();
	static
	{
		VALID_LINKS.add("common/attribute_info.htm");
		VALID_LINKS.add("common/augmentation_01.htm");
		VALID_LINKS.add("common/augmentation_02.htm");
		VALID_LINKS.add("common/augmentation_exchange.htm");
		VALID_LINKS.add("common/crafting_01.htm");
		VALID_LINKS.add("common/crafting_02.htm");
		VALID_LINKS.add("common/crafting_03.htm");
		VALID_LINKS.add("common/cursed_to_unidentified.htm");
		VALID_LINKS.add("common/duals_01.htm");
		VALID_LINKS.add("common/duals_02.htm");
		VALID_LINKS.add("common/duals_03.htm");
		VALID_LINKS.add("common/g_cube_warehouse001.htm");
		VALID_LINKS.add("common/skill_enchant_help.htm");
		VALID_LINKS.add("common/skill_enchant_help_01.htm");
		VALID_LINKS.add("common/skill_enchant_help_02.htm");
		VALID_LINKS.add("common/skill_enchant_help_03.htm");
		VALID_LINKS.add("common/smelting_trade001.htm");
		VALID_LINKS.add("common/weapon_sa_01.htm");
		VALID_LINKS.add("common/welcomeback002.htm");
		VALID_LINKS.add("common/welcomeback003.htm");
		VALID_LINKS.add("default/BlessingOfProtection.htm");
		VALID_LINKS.add("default/SupportMagic.htm");
		VALID_LINKS.add("default/SupportMagicServitor.htm");
		VALID_LINKS.add("fisherman/exchange_old_items.htm");
		VALID_LINKS.add("fisherman/fish_appearance_exchange.htm");
		VALID_LINKS.add("fisherman/fishing_manual001.htm");
		VALID_LINKS.add("fisherman/fishing_manual002.htm");
		VALID_LINKS.add("fisherman/fishing_manual003.htm");
		VALID_LINKS.add("fisherman/fishing_manual004.htm");
		VALID_LINKS.add("fisherman/fishing_manual008.htm");
		VALID_LINKS.add("fisherman/fishing_manual009.htm");
		VALID_LINKS.add("fisherman/fishing_manual010.htm");
		VALID_LINKS.add("fortress/foreman.htm");
		VALID_LINKS.add("guard/kamaloka_help.htm");
		VALID_LINKS.add("guard/kamaloka_level.htm");
		VALID_LINKS.add("petmanager/evolve.htm");
		VALID_LINKS.add("petmanager/exchange.htm");
		VALID_LINKS.add("petmanager/instructions.htm");
		VALID_LINKS.add("teleporter/separatedsoul.htm");
		VALID_LINKS.add("warehouse/clanwh.htm");
		VALID_LINKS.add("warehouse/privatewh.htm");
		// Quests
		VALID_LINKS.add("teleporter/30006.htm");
		VALID_LINKS.add("teleporter/30006-Q561.htm");
		VALID_LINKS.add("teleporter/30006-Q561-1.htm");
		VALID_LINKS.add("teleporter/30006-Q561-2.htm");
		VALID_LINKS.add("teleporter/30134.htm");
		VALID_LINKS.add("teleporter/30134-Q562.htm");
		VALID_LINKS.add("teleporter/30134-Q562-1.htm");
		VALID_LINKS.add("teleporter/30134-Q562-2.htm");
		VALID_LINKS.add("teleporter/30256.htm");
		VALID_LINKS.add("teleporter/30256-Q562.htm");
		VALID_LINKS.add("teleporter/30256-Q562-1.htm");
		VALID_LINKS.add("teleporter/30256-Q562-2.htm");
		VALID_LINKS.add("teleporter/30848.htm");
		VALID_LINKS.add("teleporter/30848-Q561-Q562.htm");
		VALID_LINKS.add("teleporter/30848-Q561-Q562-1.htm");
		VALID_LINKS.add("teleporter/30848-Q561-Q562-2.htm");
	}
	
	@Override
	public boolean onCommand(String command, Player player, Creature target)
	{
		final String htmlPath = command.substring(4).trim();
		if (htmlPath.isEmpty())
		{
			LOGGER.warning(player + " sent empty link html!");
			return false;
		}
		
		if (htmlPath.contains(".."))
		{
			LOGGER.warning(player + " sent invalid link html: " + htmlPath);
			return false;
		}
		
		String content = VALID_LINKS.contains(htmlPath) ? HtmCache.getInstance().getHtm(player, "data/html/" + htmlPath) : null;
		
		// Precaution.
		if (htmlPath.startsWith("teleporter/") && !(player.getTarget() instanceof Teleporter))
		{
			content = null;
		}
		
		final NpcHtmlMessage html = new NpcHtmlMessage(target != null ? target.getObjectId() : 0);
		if (content != null)
		{
			html.setHtml(content.replace("%objectId%", String.valueOf(target != null ? target.getObjectId() : 0)));
		}
		
		player.sendPacket(html);
		return true;
	}
	
	@Override
	public String[] getCommandList()
	{
		return COMMANDS;
	}
}
