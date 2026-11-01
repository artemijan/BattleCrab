/*
 * This file is part of the L2J Mobius project.
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */
package handlers.skill.effects;

import java.util.Collection;

import org.l2jmobius.gameserver.ai.Action;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.item.holders.ItemHolder;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Formulas;
import org.l2jmobius.gameserver.network.SystemMessageId;

/**
 * @author Sdw
 */
public class Plunder extends AbstractEffect
{
	public Plunder(StatSet params)
	{
	}
	
	@Override
	public boolean calcSuccess(Creature effector, Creature effected, Skill skill)
	{
		return Formulas.calcMagicSuccess(effector, effected, skill);
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (!effector.isPlayer())
		{
			return;
		}
		else if (!effected.isMonster())
		{
			effector.sendPacket(SystemMessageId.INVALID_TARGET);
			return;
		}
		
		final Monster monster = effected.asMonster();
		final Player player = effector.asPlayer();
		
		if (monster.isSpoiled())
		{
			effector.sendPacket(SystemMessageId.PLUNDER_SKILL_HAS_BEEN_ALREADY_USED_ON_THIS_TARGET);
			return;
		}
		
		monster.setPlundered(player);
		
		if (!player.getInventory().checkInventorySlotsAndWeight(monster.getSpoilLootItems(), false, false))
		{
			return;
		}
		
		final Collection<ItemHolder> items = monster.takeSweep();
		if (items != null)
		{
			final boolean lucky = player.tryLuck();
			for (ItemHolder sweepedItem : items)
			{
				final ItemHolder rewardedItem = new ItemHolder(sweepedItem.getId(), sweepedItem.getCount() * (lucky ? 2 : 1));
				final Party party = effector.getParty();
				if (party != null)
				{
					party.distributeItem(player, rewardedItem, true, monster);
				}
				else
				{
					player.addItem(ItemProcessType.PICKUP, rewardedItem, effected, true);
				}
			}
		}
		
		monster.getAI().notifyAction(Action.ATTACKED, effector);
	}
}
