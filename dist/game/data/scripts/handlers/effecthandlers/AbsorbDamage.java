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

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

import org.l2jmobius.gameserver.enums.StatModifierType;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.impl.creature.OnCreatureDamageReceived;
import org.l2jmobius.gameserver.model.events.listeners.FunctionEventListener;
import org.l2jmobius.gameserver.model.events.returns.DamageReturn;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Sdw, Mobius
 */
public class AbsorbDamage extends AbstractEffect
{
	private static final Map<Integer, Double> DIFF_DAMAGE_HOLDER = new ConcurrentHashMap<>();
	private static final Map<Integer, Double> PER_DAMAGE_HOLDER = new ConcurrentHashMap<>();
	
	private final double _damage;
	private final StatModifierType _mode;
	
	public AbsorbDamage(StatSet params)
	{
		_damage = params.getDouble("damage", 0);
		_mode = params.getEnum("mode", StatModifierType.class, StatModifierType.DIFF);
	}
	
	private DamageReturn onDamageReceivedDiffEvent(OnCreatureDamageReceived event, Creature effected, Skill skill)
	{
		// DOT effects are not taken into account.
		if (event.isDamageOverTime())
		{
			return null;
		}
		
		final int objectId = event.getTarget().getObjectId();
		
		final double damageLeft = DIFF_DAMAGE_HOLDER.getOrDefault(objectId, 0d);
		final double newDamageLeft = Math.max(damageLeft - event.getDamage(), 0);
		final double newDamage = Math.max(event.getDamage() - damageLeft, 0);
		
		if (newDamageLeft > 0)
		{
			DIFF_DAMAGE_HOLDER.put(objectId, newDamageLeft);
		}
		else
		{
			effected.stopSkillEffects(skill);
		}
		
		return new DamageReturn(false, true, false, newDamage);
	}
	
	private DamageReturn onDamageReceivedPerEvent(OnCreatureDamageReceived event)
	{
		// DOT effects are not taken into account.
		if (event.isDamageOverTime())
		{
			return null;
		}
		
		final int objectId = event.getTarget().getObjectId();
		
		final double damagePercent = PER_DAMAGE_HOLDER.getOrDefault(objectId, 0d);
		final double currentDamage = event.getDamage();
		final double newDamage = currentDamage - ((currentDamage / 100) * damagePercent);
		
		return new DamageReturn(false, true, false, newDamage);
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		effected.removeListenerIf(EventType.ON_CREATURE_DAMAGE_RECEIVED, listener -> listener.getOwner() == this);
		if (_mode == StatModifierType.DIFF)
		{
			DIFF_DAMAGE_HOLDER.remove(effected.getObjectId());
		}
		else
		{
			PER_DAMAGE_HOLDER.remove(effected.getObjectId());
		}
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (_mode == StatModifierType.DIFF)
		{
			DIFF_DAMAGE_HOLDER.put(effected.getObjectId(), _damage);
			effected.addListener(new FunctionEventListener(effected, EventType.ON_CREATURE_DAMAGE_RECEIVED, (OnCreatureDamageReceived event) -> onDamageReceivedDiffEvent(event, effected, skill), this));
		}
		else
		{
			PER_DAMAGE_HOLDER.put(effected.getObjectId(), _damage);
			effected.addListener(new FunctionEventListener(effected, EventType.ON_CREATURE_DAMAGE_RECEIVED, (OnCreatureDamageReceived event) -> onDamageReceivedPerEvent(event), this));
		}
	}
}
