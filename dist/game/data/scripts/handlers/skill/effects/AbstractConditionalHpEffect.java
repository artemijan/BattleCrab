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

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicBoolean;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenersContainer;
import org.l2jmobius.gameserver.model.events.holders.actor.creature.OnCreatureHpChange;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;

/**
 * @author Mobius
 */
abstract class AbstractConditionalHpEffect extends AbstractStatEffect
{
	private final int _hpPercent;
	private final Map<Creature, AtomicBoolean> _updates = new ConcurrentHashMap<>();
	
	protected AbstractConditionalHpEffect(StatSet params, Stat stat)
	{
		super(params, stat);
		_hpPercent = params.getInt("hpPercent", 0);
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		// Augmentation option
		if (skill == null)
		{
			return;
		}
		
		// Register listeners
		if ((_hpPercent > 0) && !_updates.containsKey(effected))
		{
			_updates.put(effected, new AtomicBoolean(canPump(effector, effected, skill)));
			final ListenersContainer container = effected;
			container.addListener(new ConsumerEventListener(container, EventType.ON_CREATURE_HP_CHANGE, (OnCreatureHpChange event) -> onHpChange(event), this));
		}
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		// Augmentation option
		if (skill == null)
		{
			return;
		}
		
		effected.removeListenerIf(listener -> listener.getOwner() == this);
		_updates.remove(effected);
	}
	
	@Override
	public boolean canPump(Creature effector, Creature effected, Skill skill)
	{
		return (_hpPercent <= 0) || (effected.getCurrentHpPercent() <= _hpPercent);
	}
	
	private void onHpChange(OnCreatureHpChange event)
	{
		final Creature creature = event.getCreature();
		final AtomicBoolean update = _updates.get(creature);
		if (update == null)
		{
			return;
		}
		
		if (canPump(null, creature, null))
		{
			if (update.get())
			{
				update.set(false);
				ThreadPool.execute(() -> creature.getStat().recalculateStats(true));
			}
		}
		else if (!update.get())
		{
			update.set(true);
			ThreadPool.execute(() -> creature.getStat().recalculateStats(true));
		}
	}
}
