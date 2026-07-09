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
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.effects.EffectType;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Formulas;
import org.l2jmobius.gameserver.taskmanager.DecayTaskManager;

/**
 * Resurrection effect implementation.
 * @author Adry_85
 */
public class Resurrection extends AbstractEffect
{
	private final int _power;
	private final int _hpPercent;
	private final int _mpPercent;
	private final int _cpPercent;
	
	public Resurrection(StatSet params)
	{
		_power = params.getInt("power", 0);
		_hpPercent = params.getInt("hpPercent", 0);
		_mpPercent = params.getInt("mpPercent", 0);
		_cpPercent = params.getInt("cpPercent", 0);
	}
	
	@Override
	public EffectType getEffectType()
	{
		return EffectType.RESURRECTION;
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (effector.isPlayer())
		{
			final Player player = effected.getActingPlayer();
			if (!player.isResurrectionBlocked() && !player.isReviveRequested())
			{
				effected.getActingPlayer().reviveRequest(effector.getActingPlayer(), effected.isPet(), _power, _hpPercent, _mpPercent, _cpPercent);
			}
		}
		else
		{
			DecayTaskManager.getInstance().cancel(effected);
			effected.doRevive(Formulas.calculateSkillResurrectRestorePercent(_power, effector));
		}
	}
}