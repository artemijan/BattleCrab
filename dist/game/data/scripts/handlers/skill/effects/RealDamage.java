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
package handlers.skill.effects;

import org.l2jmobius.gameserver.config.custom.FakePlayersConfig;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.StatModifierType;
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
		_power = params.getDouble("power", 0);
		_mode = params.getEnum("mode", StatModifierType.class, StatModifierType.DIFF);
		
		if (params.contains("amount"))
		{
			throw new IllegalArgumentException(getClass().getSimpleName() + " should use power instead of amount.");
		}
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
		
		// Check if effected is Invul (Celestial) or HP Blocked (Petrification).
		if (effected.isHpBlocked() || effected.isInvul())
		{
			return;
		}
		
		// Check if effected NPC is not attackable.
		if (effected.isNpc() && !effected.isAttackable())
		{
			return;
		}
		
		// Check if fake players should aggro each other.
		if (effector.isFakePlayer() && !FakePlayersConfig.FAKE_PLAYER_AGGRO_FPC && effected.isFakePlayer())
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
			damage = (effected.getCurrentHp() * _power) / 100;
		}
		
		// Do damage.
		if (damage > 0)
		{
			effected.setCurrentHp(Math.max(effected.getCurrentHp() - damage, effected.isUndying() ? 1 : 0));
			
			// Die.
			if ((effected.getCurrentHp() < 0.5) && (!effected.isPlayer() || !effected.asPlayer().isInOlympiadMode()))
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
