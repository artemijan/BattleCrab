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
package handlers.items;

import org.l2jmobius.gameserver.handler.IItemHandler;
import org.l2jmobius.gameserver.managers.HandysBlockCheckerManager;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Playable;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.holders.player.ArenaParticipantsHolder;
import org.l2jmobius.gameserver.model.actor.instance.Block;
import org.l2jmobius.gameserver.model.item.enums.ItemSkillType;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;

public class EventItem implements IItemHandler
{
	@Override
	public boolean onItemUse(Playable playable, Item item, boolean forceUse)
	{
		if (!playable.isPlayer())
		{
			playable.sendPacket(SystemMessageId.YOUR_PET_CANNOT_CARRY_THIS_ITEM);
			return false;
		}
		
		boolean used = false;
		
		final Player player = playable.asPlayer();
		final int itemId = item.getId();
		switch (itemId)
		{
			case 13787: // Handy's Block Checker Bond
			{
				used = useBlockCheckerItem(player, item);
				break;
			}
			case 13788: // Handy's Block Checker Land Mine
			{
				used = useBlockCheckerItem(player, item);
				break;
			}
			default:
			{
				LOGGER.warning("EventItemHandler: Item with id: " + itemId + " is not handled");
			}
		}
		
		return used;
	}
	
	private final boolean useBlockCheckerItem(Player caster, Item item)
	{
		final int blockCheckerArena = caster.getBlockCheckerArena();
		if (blockCheckerArena == -1)
		{
			final SystemMessage msg = new SystemMessage(SystemMessageId.S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS);
			msg.addItemName(item);
			caster.sendPacket(msg);
			return false;
		}
		
		final Skill sk = item.getEtcItem().getSkills(ItemSkillType.NORMAL).get(0).getSkill();
		if (sk == null)
		{
			return false;
		}
		
		if (!caster.destroyItem(null, item, 1, caster, true))
		{
			return false;
		}
		
		final Block block = (Block) caster.getTarget();
		final ArenaParticipantsHolder holder = HandysBlockCheckerManager.getInstance().getHolder(blockCheckerArena);
		if (holder != null)
		{
			final int team = holder.getPlayerTeam(caster);
			World.getInstance().forEachVisibleObjectInRange(block, Player.class, sk.getEffectRange(), pc ->
			{
				final int enemyTeam = holder.getPlayerTeam(pc);
				if ((enemyTeam != -1) && (enemyTeam != team))
				{
					sk.applyEffects(caster, pc);
				}
			});
			
			return true;
		}
		
		LOGGER.warning("Char: " + caster.getName() + "[" + caster.getObjectId() + "] has unknown block checker arena");
		return false;
	}
}
