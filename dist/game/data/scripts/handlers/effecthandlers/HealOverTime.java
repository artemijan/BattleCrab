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
package handlers.effecthandlers;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.AbnormalType;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;
import org.l2jmobius.gameserver.network.serverpackets.ExRegenMax;

/**
 * Heal Over Time effect implementation.
 */
public class HealOverTime extends AbstractEffect
{
	private final double _power;
	
	public HealOverTime(StatSet params)
	{
		_power = params.getDouble("power", 0);
		setTicks(params.getInt("ticks"));
	}
	
	@Override
	public boolean onActionTime(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effected.isDead() || effected.isDoor())
		{
			return false;
		}
		
		double hp = effected.getCurrentHp();
		final double maxhp = effected.getMaxRecoverableHp();
		
		// Not needed to set the HP and send update packet if player is already at max HP
		if (_power > 0)
		{
			if (hp >= maxhp)
			{
				return false;
			}
		}
		else
		{
			if ((hp - _power) <= 0)
			{
				return false;
			}
		}
		
		double power = _power;
		if ((item != null) && (item.isPotion() || item.isElixir()))
		{
			power += effected.getStat().getValue(Stat.ADDITIONAL_POTION_HP, 0) / getTicks();
		}
		
		hp += power * getTicksMultiplier();
		if (_power > 0)
		{
			hp = Math.min(hp, maxhp);
		}
		else
		{
			hp = Math.max(hp, 1);
		}
		effected.setCurrentHp(hp, false);
		effected.broadcastStatusUpdate(effector);
		return skill.isToggle();
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effected.isPlayer() && (getTicks() > 0) && (skill.getAbnormalType() == AbnormalType.HP_RECOVER))
		{
			double power = _power;
			if ((item != null) && (item.isPotion() || item.isElixir()))
			{
				final double bonus = effected.getStat().getValue(Stat.ADDITIONAL_POTION_HP, 0);
				if (bonus > 0)
				{
					power += bonus / getTicks();
				}
			}
			
			effected.sendPacket(new ExRegenMax(skill.getAbnormalTime(), getTicks(), power));
		}
	}
}
