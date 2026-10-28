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

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerCheatDeath;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * @author Mobius
 */
public class CheatDeath extends AbstractEffect
{
	private final int _hpFixed;
	private final int _mpFixed;
	private final int _cpFixed;
	private final int _hpPercent;
	private final int _mpPercent;
	private final int _cpPercent;
	private final boolean _removed;
	private final List<SkillHolder> _triggerSkill;
	
	public CheatDeath(StatSet params)
	{
		_hpFixed = params.getInt("hp", -1);
		_mpFixed = params.getInt("mp", -1);
		_cpFixed = params.getInt("cp", -1);
		_hpPercent = params.getInt("hpPercent", -1);
		_mpPercent = params.getInt("mpPercent", -1);
		_cpPercent = params.getInt("cpPercent", -1);
		_removed = params.getBoolean("removed", true);
		
		final String triggerSkills = params.getString("triggerSkill", "");
		if (triggerSkills.isEmpty())
		{
			_triggerSkill = null;
		}
		else
		{
			final String[] split = triggerSkills.split(";");
			_triggerSkill = new ArrayList<>(split.length);
			
			for (String skill : split)
			{
				final String[] splitSkill = skill.split(",");
				if (splitSkill.length == 1)
				{
					_triggerSkill.add(new SkillHolder(Integer.parseInt(splitSkill[0]), 1));
				}
				else
				{
					_triggerSkill.add(new SkillHolder(Integer.parseInt(splitSkill[0]), Integer.parseInt(splitSkill[1])));
				}
			}
		}
	}
	
	@Override
	public void onStart(Creature effector, Creature effected, Skill skill, Item item)
	{
		effected.addListener(new ConsumerEventListener(effected, EventType.ON_PLAYER_CHEAT_DEATH, (OnPlayerCheatDeath event) -> onPlayerCheatDeath(event.getPlayer(), skill), this));
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		effected.removeListenerIf(EventType.ON_PLAYER_CHEAT_DEATH, listener -> listener.getOwner() == this);
	}
	
	private void onPlayerCheatDeath(Player player, Skill skill)
	{
		if (_hpFixed > 0)
		{
			player.setCurrentHp(_hpFixed);
		}
		
		if (_mpFixed > -1)
		{
			player.setCurrentMp(_mpFixed);
		}
		
		if (_cpFixed > -1)
		{
			player.setCurrentCp(_cpFixed);
		}
		
		if (_hpPercent > 0)
		{
			player.setCurrentHp(player.getMaxRecoverableHp() * (_hpPercent / 100));
		}
		
		if (_mpPercent > -1)
		{
			player.setCurrentMp(player.getMaxRecoverableMp() * (_mpPercent / 100));
		}
		
		if (_cpPercent > -1)
		{
			player.setCurrentCp(player.getMaxRecoverableCp() * (_cpPercent / 100));
		}
		
		if (_triggerSkill != null)
		{
			for (SkillHolder holder : _triggerSkill)
			{
				SkillCaster.triggerCast(player, player, holder.getSkill());
			}
		}
		
		if (_removed && !skill.isPassive())
		{
			player.stopSkillEffects(skill);
		}
	}
}
