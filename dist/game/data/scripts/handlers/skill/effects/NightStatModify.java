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

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.events.Containers;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenersContainer;
import org.l2jmobius.gameserver.model.events.holders.OnDayNightChange;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.StatModifierType;
import org.l2jmobius.gameserver.model.stats.Stat;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.SystemMessage;
import org.l2jmobius.gameserver.taskmanagers.GameTimeTaskManager;

/**
 * @author Mobius
 */
public class NightStatModify extends AbstractEffect
{
	private static final Set<Creature> NIGHT_STAT_CHARACTERS = ConcurrentHashMap.newKeySet();
	private static final int SHADOW_SENSE = 294;
	
	private static boolean START_LISTENER = true;
	
	private final Stat _stat;
	private final Double _amount;
	protected final StatModifierType _mode;
	
	public NightStatModify(StatSet params)
	{
		_stat = params.getEnum("stat", Stat.class);
		_amount = params.getDouble("amount");
		_mode = params.getEnum("mode", StatModifierType.class, StatModifierType.DIFF);
		
		// Run only once per daytime change.
		if (START_LISTENER)
		{
			START_LISTENER = false;
			
			// Init a global day-night change listener.
			final ListenersContainer container = Containers.Global();
			container.addListener(new ConsumerEventListener(container, EventType.ON_DAY_NIGHT_CHANGE, (OnDayNightChange event) -> onDayNightChange(event), this));
		}
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		NIGHT_STAT_CHARACTERS.add(effected);
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		NIGHT_STAT_CHARACTERS.remove(effected);
	}
	
	@Override
	public void pump(Creature effected, Skill skill)
	{
		// Not night.
		if (!GameTimeTaskManager.getInstance().isNight())
		{
			return;
		}
		
		// Apply stat.
		switch (_mode)
		{
			case DIFF:
			{
				effected.getStat().mergeAdd(_stat, _amount);
				break;
			}
			case PER:
			{
				effected.getStat().mergeMul(_stat, (_amount / 100) + 1);
				break;
			}
		}
	}
	
	public void onDayNightChange(OnDayNightChange event)
	{
		// System message for Shadow Sense.
		final SystemMessage msg = new SystemMessage(event.isNight() ? SystemMessageId.IT_IS_NOW_MIDNIGHT_AND_THE_EFFECT_OF_S1_CAN_BE_FELT : SystemMessageId.IT_IS_DAWN_AND_THE_EFFECT_OF_S1_WILL_NOW_DISAPPEAR);
		msg.addSkillName(SHADOW_SENSE);
		
		for (Creature creature : NIGHT_STAT_CHARACTERS)
		{
			// Pump again.
			creature.getStat().recalculateStats(true);
			
			// Send Shadow Sense message when player has skill.
			if (creature.getKnownSkill(SHADOW_SENSE) != null)
			{
				creature.sendPacket(msg);
			}
		}
	}
}
