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

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.impl.creature.OnCreatureSkillUse;
import org.l2jmobius.gameserver.model.events.listeners.FunctionEventListener;
import org.l2jmobius.gameserver.model.events.returns.TerminateReturn;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * Block Skills by isMagic type or skill id.
 *
 * @author Nik, Mobius
 */
public class BlockSkill extends AbstractEffect {
  private final Set<Integer> _magicTypes = new HashSet<>();
  private final Set<Integer> _skillIds = new HashSet<>();

  public BlockSkill(StatSet params) {
    if (params.contains("magicTypes")) {
      for (int id : params.getIntArray("magicTypes", ";")) {
        _magicTypes.add(id);
      }
    }
    if (params.contains("skillIds")) {
      for (int id : params.getIntArray("skillIds", ";")) {
        _skillIds.add(id);
      }
    }
  }

  @Override
  public void onStart(Creature effector, Creature effected, Skill skill, Item item) {
    if (_magicTypes.isEmpty() && _skillIds.isEmpty()) {
      return;
    }

    effected.addListener(new FunctionEventListener(effected, EventType.ON_CREATURE_SKILL_USE, (OnCreatureSkillUse event) -> onSkillUseEvent(event), this));
  }

  @Override
  public void onExit(Creature effector, Creature effected, Skill skill) {
    effected.removeListenerIf(EventType.ON_CREATURE_SKILL_USE, listener -> listener.getOwner() == this);
  }

  private TerminateReturn onSkillUseEvent(OnCreatureSkillUse event) {
    if (_magicTypes.contains(event.getSkill().getMagicType()) || _skillIds.contains(event.getSkill().getId())) {
      return new TerminateReturn(true, true, true);
    }

    return null;
  }
}
