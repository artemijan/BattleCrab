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

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.enums.StatModifierType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author Sdw, Mobius
 */
public class RealDamage extends AbstractEffect
{
	private final double _power;
	private final StatModifierType _mode;
	
	public RealDamage(StatSet params)
	{
		if (params.getDouble("amount", 0) > 0)
		{
			LOGGER.warning(getClass().getSimpleName() + " should use power instead of amount.");
		}
		_power = params.getDouble("power", 0);
		_mode = params.getEnum("mode", StatModifierType.class, StatModifierType.DIFF);
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effected.isDead() || effected.isDoor() || effected.isRaid())
		{
			return;
		}
		
		// Check if effected NPC is not attackable.
		if (effected.isNpc() && !effected.isAttackable())
		{
			return;
		}
		
		// Check if fake players should aggro each other.
		if (effector.isFakePlayer() && !Config.FAKE_PLAYER_AGGRO_FPC && effected.isFakePlayer())
		{
			return;
		}
		
		// Calculate resistance.
		final double damage;
		if (_mode == StatModifierType.DIFF)
		{
			damage = _power - (_power * (Math.min(effected.getStat().getMulValue(Stat.REAL_DAMAGE_RESIST, 1), 1.8) - 1));
		}
		else // PER
		{
			// Percent does not ignore HP block.
			if (effected.isHpBlocked())
			{
				return;
			}
			
			damage = (effected.getCurrentHp() * _power) / 100;
		}
		
		// Do damage.
		if (damage > 0)
		{
			effected.setCurrentHp(Math.max(effected.getCurrentHp() - damage, effected.isUndying() ? 1 : 0));
			
			// Die.
			if ((effected.getCurrentHp() < 0.5) && (!effected.isPlayer() || !effected.getActingPlayer().isInOlympiadMode()))
			{
				effected.doDie(effector);
			}
		}
		
		// Send message.
		if (effector.isPlayer())
		{
			effector.sendDamageMessage(effected, skill, (int) damage, false, false);
		}
	}
}
