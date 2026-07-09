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
package ai.others;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.instancemanager.CastleManager;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Attackable;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Playable;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.item.type.WeaponType;
import org.l2jmobius.gameserver.model.siege.Castle;
import org.l2jmobius.gameserver.model.siege.Fort;
import org.l2jmobius.gameserver.model.skill.Skill;

import ai.AbstractNpcAI;

/**
 * @author Mobius
 */
public class SiegeGuards extends AbstractNpcAI
{
	//@formatter:off
	// NPCs
	private static final int[] CASTLE_GUARDS = 
	{
		35064, 35065, 35066, 35067, 35068, 35069, 35071, 35072, 35079, 35080, 35081, 35082, 35083, 35084, 35085, // Gludio
		35106, 35107, 35108, 35109, 35110, 35111, 35113, 35114, 35121, 35122, 35123,35124, 35125, 35126, 35127, // Dion
		35150, 35151, 35152, 35153, 35155, 35156, 35163, 35164, 35165, 35166, 35167, 35168, 35169, // Giran
		35192, 35193, 35194, 35195, 35197, 35198, 35205, 35206, 35207, 35208, 35209, 35210, 35211, // Oren
		35234, 35239, 35240, 35248, 35249, 35250, 35251, 35252, 35253, 35254, // Aden
		35280, 35281, 35282, 35283, 35284, 35285, 35287, 35288, 35295, 35296, 35297, 35298, 35299, 35300, 35301, // Innadril
		35324, 35325, 35326, 35327, 35328, 35330, 35339, 35340, 35341, 35343, 35350, 35351, // Goddard
		35475, 35477, 35480, 35484, 35486, 35487, 35488, 35489, 35490, // Rune
		35516, 35517, 35518, 35519, 35520, 35522, 35531, 35532, 35533, 35535, 35542, 35543, // Schuttgart
	};
	private static final int[] MERCENARIES =
	{
		35015, 35016, 35017, 35018, 35019, 35025, 35026, 35027, 35028, 35029, 35035, 35036, 35037, 35038, 35039, 35045, 35046, 35047, 35048, 35049, 35055, 35056, 35057, 35058, 35059, 35060, 35061
	};
	private static final int[] STATIONARY_MERCENARIES =
	{
		35010, 35011, 35012, 35013, 35014, 35020, 35021, 35022, 35023, 35024, 35030, 35031, 35032, 35033, 35034, 35040, 35041, 35042, 35043, 35044, 35050, 35051, 35052, 35053, 35054, 35092, 35093, 35094,
		35134, 35135, 35136, 35176, 35177, 35178, 35218, 35219, 35220, 35261, 35262, 35263, 35264, 35265, 35308, 35309, 35310, 35352, 35353, 35354, 35497, 35498, 35499, 35500, 35501, 35544, 35545, 35546
	};
	//@formatter:on
	@SuppressWarnings("unchecked")
	protected static final List<Npc>[] RESIDENCE_GUARD_MAP = new CopyOnWriteArrayList[122];
	protected static final boolean[] RESIDENCE_WORKING = new boolean[122];
	
	public SiegeGuards()
	{
		addAttackId(CASTLE_GUARDS);
		addAttackId(MERCENARIES);
		addAttackId(STATIONARY_MERCENARIES);
		addSpawnId(CASTLE_GUARDS);
		addSpawnId(MERCENARIES);
		addSpawnId(STATIONARY_MERCENARIES);
		addKillId(CASTLE_GUARDS);
		addKillId(MERCENARIES);
		addKillId(STATIONARY_MERCENARIES);
		
		// Start task for unknown residences.
		RESIDENCE_GUARD_MAP[0] = new CopyOnWriteArrayList<>();
		ThreadPool.scheduleAtFixedRate(new AggroCheckTask(0), 0, 3000);
		
		// Start tasks for castles.
		for (Castle castle : CastleManager.getInstance().getCastles())
		{
			final int residenceId = castle.getResidenceId();
			RESIDENCE_GUARD_MAP[residenceId] = new CopyOnWriteArrayList<>();
			ThreadPool.scheduleAtFixedRate(new AggroCheckTask(residenceId), residenceId * 100L, 3000);
		}
	}
	
	private class AggroCheckTask implements Runnable
	{
		private final int _residenceId;
		
		public AggroCheckTask(int residenceId)
		{
			_residenceId = residenceId;
		}
		
		@Override
		public void run()
		{
			synchronized (RESIDENCE_WORKING)
			{
				if (RESIDENCE_WORKING[_residenceId])
				{
					return;
				}
				RESIDENCE_WORKING[_residenceId] = true;
			}
			
			final List<Npc> guards = RESIDENCE_GUARD_MAP[_residenceId];
			for (Npc guard : guards)
			{
				// Should never happen.
				if ((guard == null) || !guard.isAttackable())
				{
					continue;
				}
				
				// Remove dead guards.
				if (guard.isDead())
				{
					guards.remove(guard);
					continue;
				}
				
				// Skip if guard is currently attacking.
				if (guard.isInCombat())
				{
					continue;
				}
				
				// Skip if guard has an active target (not dead/invis/invul and is within range).
				final WorldObject target = guard.getTarget();
				final Creature targetCreature = (target != null) && target.isCreature() ? (Creature) target : null;
				if ((targetCreature != null) && !targetCreature.isDead() && !targetCreature.isInvisible() && !targetCreature.isInvul() && (guard.calculateDistance2D(targetCreature) < guard.getAggroRange()))
				{
					continue;
				}
				
				// Iterate all players/summons within aggro range...
				for (Playable nearby : World.getInstance().getVisibleObjectsInRange(guard, Playable.class, guard.getAggroRange()))
				{
					// Do not attack players/summons who are dead/invis/invul or cannot be seen.
					if (nearby.isDead() || nearby.isInvisible() || nearby.isInvul() || !GeoEngine.getInstance().canSeeTarget(guard, nearby))
					{
						continue;
					}
					
					// Do not attack defenders who are registered to this castle.
					final Player player = nearby.getActingPlayer();
					if ((player.getSiegeState() == 2) && player.isRegisteredOnThisSiegeField(guard.getScriptValue()))
					{
						continue;
					}
					
					// Attack the target and stop searching.
					((Attackable) guard).addDamageHate(nearby, 0, 999);
					break;
				}
			}
			
			synchronized (RESIDENCE_WORKING)
			{
				RESIDENCE_WORKING[_residenceId] = false;
			}
		}
	}
	
	@Override
	public String onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		if ((attacker.getSiegeState() == 2) && !attacker.isRegisteredOnThisSiegeField(npc.getScriptValue()))
		{
			((Attackable) npc).stopHating(attacker);
			return null;
		}
		return super.onAttack(npc, attacker, damage, isSummon, skill);
	}
	
	@Override
	public String onKill(Npc npc, Player killer, boolean isSummon)
	{
		final int residenceId = npc.getScriptValue();
		final List<Npc> guardList = RESIDENCE_GUARD_MAP[residenceId];
		if (guardList != null)
		{
			guardList.remove(npc);
		}
		else
		{
			RESIDENCE_GUARD_MAP[0].remove(npc);
		}
		return super.onKill(npc, killer, isSummon);
	}
	
	@Override
	public String onSpawn(Npc npc)
	{
		npc.setRandomWalking(false);
		if ((npc.getTemplate().getBaseAttackType() != WeaponType.SWORD) && (npc.getTemplate().getBaseAttackType() != WeaponType.POLE))
		{
			npc.setImmobilized(true);
		}
		
		final Castle castle = npc.getCastle();
		final Fort fortress = npc.getFort();
		final int residenceId = fortress != null ? fortress.getResidenceId() : (castle != null ? castle.getResidenceId() : 0);
		npc.setScriptValue(residenceId);
		final List<Npc> guardList = RESIDENCE_GUARD_MAP[residenceId];
		if (guardList != null)
		{
			guardList.add(npc);
		}
		else // Residence id not found.
		{
			RESIDENCE_GUARD_MAP[0].add(npc);
		}
		
		return super.onSpawn(npc);
	}
	
	public static void main(String[] args)
	{
		new SiegeGuards();
	}
}
