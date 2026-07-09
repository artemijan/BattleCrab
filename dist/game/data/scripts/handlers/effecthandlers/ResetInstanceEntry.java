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

import java.util.Collections;
import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.instancemanager.InstanceManager;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Mobius
 */
public class ResetInstanceEntry extends AbstractEffect
{
	private final Set<Integer> _instanceId;
	
	public ResetInstanceEntry(StatSet params)
	{
		final String instanceIds = params.getString("instanceId", null);
		if ((instanceIds != null) && !instanceIds.isEmpty())
		{
			_instanceId = new HashSet<>();
			for (String id : instanceIds.split(";"))
			{
				_instanceId.add(Integer.parseInt(id));
			}
		}
		else
		{
			_instanceId = Collections.emptySet();
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
		for (int instanceId : _instanceId)
		{
			InstanceManager.getInstance().deleteInstanceTime(effector.getActingPlayer(), instanceId);
			final String instanceName = InstanceManager.getInstance().getInstanceName(instanceId);
			effector.sendMessage(instanceName + " entry has been reset.");
		}
	}
}
