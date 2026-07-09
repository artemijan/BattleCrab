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

import java.util.logging.Logger;

import org.l2jmobius.commons.util.Rnd;
import org.l2jmobius.gameserver.data.xml.CubicData;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.templates.CubicTemplate;
import org.l2jmobius.gameserver.model.cubic.Cubic;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.stats.Stat;
import org.l2jmobius.gameserver.network.serverpackets.ExUserInfoCubic;

/**
 * Summon Cubic effect implementation.
 * @author Zoey76
 */
public class SummonCubic extends AbstractEffect
{
	private static final Logger LOGGER = Logger.getLogger(SummonCubic.class.getName());
	
	private final int _cubicId;
	private final int _cubicLvl;
	
	public SummonCubic(StatSet params)
	{
		_cubicId = params.getInt("cubicId", -1);
		_cubicLvl = params.getInt("cubicLvl", 0);
	}
	
	@Override
	public boolean isInstant()
	{
		return true;
	}
	
	@Override
	public void instant(Creature effector, Creature effected, Skill skill, Item item)
	{
		if (!effected.isPlayer() || effected.isAlikeDead() || effected.getActingPlayer().inObserverMode())
		{
			return;
		}
		
		if (_cubicId < 0)
		{
			LOGGER.warning(SummonCubic.class.getSimpleName() + ": Invalid Cubic ID:" + _cubicId + " in skill ID: " + skill.getId());
			return;
		}
		
		final Player player = effected.getActingPlayer();
		if (player.inObserverMode() || player.isMounted())
		{
			return;
		}
		
		// If cubic is already present, it's replaced.
		final Cubic cubic = player.getCubicById(_cubicId);
		if (cubic != null)
		{
			if (cubic.getTemplate().getLevel() > _cubicLvl)
			{
				// What do we do in such case?
				return;
			}
			
			cubic.deactivate();
		}
		else
		{
			// If maximum amount is reached, random cubic is removed.
			// Players with no mastery can have only one cubic.
			final double allowedCubicCount = player.getStat().getValue(Stat.MAX_CUBIC, 1);
			
			// Extra cubics are removed, one by one, randomly.
			final int currentCubicCount = player.getCubics().size();
			if (currentCubicCount >= allowedCubicCount)
			{
				player.getCubics().values().stream().skip((int) (currentCubicCount * Rnd.nextDouble())).findAny().get().deactivate();
			}
		}
		
		final CubicTemplate template = CubicData.getInstance().getCubicTemplate(_cubicId, _cubicLvl);
		if (template == null)
		{
			LOGGER.warning("Attempting to summon cubic without existing template id: " + _cubicId + " level: " + _cubicLvl);
			return;
		}
		
		// Adding a new cubic.
		player.addCubic(new Cubic(player, effector.getActingPlayer(), template));
		player.sendPacket(new ExUserInfoCubic(player));
		player.broadcastCharInfo();
	}
}
