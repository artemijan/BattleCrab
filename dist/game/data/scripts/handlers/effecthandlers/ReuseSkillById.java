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
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.serverpackets.SkillCoolTime;

/**
 * @author Mobius
 */
public class ReuseSkillById extends AbstractEffect
{
	private final int _skillId;
	private final int _amount;
	
	public ReuseSkillById(StatSet params)
	{
		_skillId = params.getInt("skillId", 0);
		_amount = params.getInt("amount", 0);
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		final Player player = effector.getActingPlayer();
		if (player != null)
		{
			final Skill s = player.getKnownSkill(_skillId);
			if (s != null)
			{
				if (_amount > 0)
				{
					final long reuse = player.getSkillRemainingReuseTime(s.getReuseHashCode());
					if (reuse > 0)
					{
						player.removeTimeStamp(s);
						player.addTimeStamp(s, Math.max(0, reuse - _amount));
						player.sendPacket(new SkillCoolTime(player));
					}
				}
				else
				{
					player.removeTimeStamp(s);
					player.enableSkill(s);
					player.sendPacket(new SkillCoolTime(player));
				}
			}
		}
	}
}
