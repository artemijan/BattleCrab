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
package handlers.playeractions;

import org.l2jmobius.gameserver.ai.CtrlIntention;
import org.l2jmobius.gameserver.handler.IPlayerActionHandler;
import org.l2jmobius.gameserver.model.ActionDataHolder;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Pet;
import org.l2jmobius.gameserver.network.SystemMessageId;

/**
 * Pet attack player action handler.
 *
 * @author Mobius
 */
public class PetAttack implements IPlayerActionHandler {
  @Override
  public void useAction(Player player, ActionDataHolder data, boolean ctrlPressed, boolean shiftPressed) {
    final Pet pet = player.getPet();
    if ((pet == null) || !pet.isPet()) {
      player.sendPacket(SystemMessageId.YOU_DO_NOT_HAVE_A_PET);
      return;
    }

    if (pet.isUncontrollable()) {
      player.sendPacket(SystemMessageId.WHEN_YOUR_PET_S_HUNGER_GAUGE_IS_AT_0_YOU_CANNOT_USE_YOUR_PET);
      return;
    }

    if (pet.isBetrayed()) {
      player.sendPacket(SystemMessageId.YOUR_SERVITOR_IS_UNRESPONSIVE_AND_WILL_NOT_OBEY_ANY_ORDERS);
      return;
    }

    final WorldObject target = player.getTarget();
    if (target == null) {
      return;
    }

    if (player.calculateDistance3D(target) > 3000) {
      pet.getAI().setIntention(CtrlIntention.AI_INTENTION_FOLLOW, player);
    } else if (pet.canAttack(target, ctrlPressed)) {
      pet.doAttack(target);
    }
  }
}
