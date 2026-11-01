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
package ai.others;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.managers.CastleManager;
import org.l2jmobius.gameserver.managers.FortManager;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.WorldObject;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Playable;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.sieges.OnCastleSiegeFinish;
import org.l2jmobius.gameserver.model.events.holders.sieges.OnFortSiegeFinish;
import org.l2jmobius.gameserver.model.item.type.WeaponType;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.siege.Castle;
import org.l2jmobius.gameserver.model.siege.Fort;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.spawns.Spawn;

/**
 * @author Mobius
 */
public class SiegeGuards extends Script
{
	// @formatter:off
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
	private static final int[] FORT_GUARDS = 
	{
		35670, 35671, 35672, 35673, 35674, 35678, 35679, 35681, 35682, 35684, 35685, // Shanty
		35702, 35703, 35704, 35705, 35706, 35707, 35708, 35709, 35710, 35711, 35712, 35714, 35715, 35717, 35718, 35720, 35722, 35723, // Southern
		35739, 35740, 35741, 35742, 35743, 35747, 35748, 35750, 35751, 35753, 35754, // Hive
		35758, 35767, 35771, 35772, 35773, 35774, 35775, 35776, 35777, 35778, 35779, 35780, 35781, 35783, 35784, 35786, 35787, 35789, 35791, 35792, // Valley
		35808, 35809, 35810, 35811, 35812, 35816, 35817, 35819, 35820, 35822, 35823, // Ivory
		35839, 35840, 35841, 35842, 35843, 35847, 35848, 35850, 35851, 35853, 35854, // Narsell
		35868, 35869, 35871, 35872, 35873, 35874, 35875, 35876, 35877, 35878, 35879, 35880, 35881, 35883, 35884, 35886, 35887, 35889, 35891, 35892, // Bayou
		35908, 35909, 35910, 35911, 35912, 35916, 35917, 35919, 35920, 35922, 35923, // White Sands
		35940, 35941, 35942, 35943, 35944, 35945, 35946, 35947, 35948, 35949, 35950, 35952, 35953, 35955, 35956, 35958, 35960, 35961, // Borderland
		35978, 35979, 35980, 35981, 35982, 35983, 35984, 35985, 35986, 35987, 35988, 35990, 35991, 35993, 35994, 35996, 35998, 35999, // Swamp
		36015, 36016, 36017, 36018, 36019, 36023, 36024, 36026, 36027, 36029, 36030, // Archaic
		36047, 36048, 36049, 36050, 36051, 36052, 36053, 36054, 36055, 36056, 36057, 36059, 36060, 36062, 36063, 36065, 36067, 36068, 36468, // Floran
		36079, 36085, 36086, 36087, 36088, 36089, 36090, 36091, 36092, 36093, 36094, 36095, 36097, 36098, 36100, 36101, 36103, 36105, 36106, // Removed on Helios (113)
		36122, 36123, 36124, 36125, 36126, 36130, 36131, 36133, 36134, 36136, 36137, // Tanor
		36153, 36154, 36155, 36156, 36157, 36161, 36162, 36164, 36165, 36167, 36168, // Dragonspine
		36185, 36186, 36187, 36188, 36189, 36190, 36191, 36192, 36193, 36194, 36195, 36197, 36198, 36200, 36201, 36203, 36205, 36206, // Antharas
		36223, 36224, 36225, 36226, 36227, 36228, 36229, 36230, 36231, 36232, 36233, 36235, 36236, 36238, 36239, 36241, 36243, 36244, // Western
		36261, 36262, 36263, 36264, 36265, 36266, 36267, 36268, 36269, 36270, 36271, 36273, 36274, 36276, 36277, 36279, 36281, 36282, // Hunters
		36298, 36299, 36300, 36301, 36302, 36306, 36307, 36309, 36310, 36312, 36313, // Aaru
		36330, 36331, 36332, 36333, 36334, 36342, 36343, 36345, 36346, 36348, 36351, // Demon
		36368, 36369, 36370, 36371, 36372, 36380, 36381, 36383, 36384, 36386, 36389 // Monastic
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
	// @formatter:on
	@SuppressWarnings("unchecked")
	protected static final List<Npc>[] RESIDENCE_GUARD_MAP = new CopyOnWriteArrayList[122];
	protected static final boolean[] RESIDENCE_WORKING = new boolean[122];
	
	public SiegeGuards()
	{
		addAttackId(CASTLE_GUARDS);
		addAttackId(FORT_GUARDS);
		addAttackId(MERCENARIES);
		addAttackId(STATIONARY_MERCENARIES);
		addSpawnId(CASTLE_GUARDS);
		addSpawnId(FORT_GUARDS);
		addSpawnId(MERCENARIES);
		addSpawnId(STATIONARY_MERCENARIES);
		addKillId(CASTLE_GUARDS);
		addKillId(FORT_GUARDS);
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
			ThreadPool.scheduleAtFixedRate(new AggroCheckTask(residenceId), residenceId * 100, 3000);
		}
		
		// Start tasks for fortresses.
		for (Fort fort : FortManager.getInstance().getForts())
		{
			final int residenceId = fort.getResidenceId();
			RESIDENCE_GUARD_MAP[residenceId] = new CopyOnWriteArrayList<>();
			ThreadPool.scheduleAtFixedRate(new AggroCheckTask(residenceId), 900 + ((residenceId - 100) * 100), 3000);
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
				
				// Ignore dead guards.
				if (guard.isDead())
				{
					continue;
				}
				
				// Skip if guard is currently attacking.
				if (guard.isInCombat())
				{
					continue;
				}
				
				// Skip if guard has an active target (not dead/invis/invul and is within range).
				final WorldObject target = guard.getTarget();
				final Creature targetCreature = (target != null) && target.isCreature() ? target.asCreature() : null;
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
					final Player player = nearby.asPlayer();
					if ((player.getSiegeState() == 2) && player.isRegisteredOnThisSiegeField(guard.getScriptValue()))
					{
						continue;
					}
					
					// Attack the target and stop searching.
					guard.asAttackable().addDamageHate(nearby, 0, 999);
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
	public void onAttack(Npc npc, Player attacker, int damage, boolean isSummon, Skill skill)
	{
		if ((attacker.getSiegeState() == 2) && !attacker.isRegisteredOnThisSiegeField(npc.getScriptValue()))
		{
			npc.asAttackable().stopHating(attacker);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final int residenceId = npc.getScriptValue();
		if (residenceId == 0)
		{
			RESIDENCE_GUARD_MAP[0].remove(npc);
		}
	}
	
	@Override
	public void onSpawn(Npc npc)
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
		if ((residenceId > 0) && (guardList != null))
		{
			if (!guardList.contains(npc))
			{
				guardList.add(npc);
			}
		}
		else // Residence id not found.
		{
			RESIDENCE_GUARD_MAP[0].add(npc);
		}
	}
	
	@RegisterEvent(EventType.ON_CASTLE_SIEGE_FINISH)
	@RegisterType(ListenerRegisterType.GLOBAL)
	public void onCastleSiegeFinish(OnCastleSiegeFinish event)
	{
		final Castle castle = event.getSiege().getCastle();
		final int residenceId = castle != null ? castle.getResidenceId() : 0;
		final List<Npc> guardList = RESIDENCE_GUARD_MAP[residenceId];
		if (guardList == null)
		{
			return;
		}
		
		for (Npc guard : guardList)
		{
			final Spawn spawn = guard.getSpawn();
			if (spawn != null)
			{
				spawn.stopRespawn();
			}
			
			if (guard.isSpawned())
			{
				guard.deleteMe();
			}
		}
		
		guardList.clear();
	}
	
	@RegisterEvent(EventType.ON_FORT_SIEGE_FINISH)
	@RegisterType(ListenerRegisterType.GLOBAL)
	public void onFortSiegeFinish(OnFortSiegeFinish event)
	{
		final Fort fort = event.getSiege().getFort();
		final int residenceId = fort != null ? fort.getResidenceId() : 0;
		final List<Npc> guardList = RESIDENCE_GUARD_MAP[residenceId];
		if (guardList == null)
		{
			return;
		}
		
		for (Npc guard : guardList)
		{
			final Spawn spawn = guard.getSpawn();
			if (spawn != null)
			{
				spawn.stopRespawn();
			}
			
			if (guard.isSpawned())
			{
				guard.deleteMe();
			}
		}
		
		guardList.clear();
	}
	
	public static void main(String[] args)
	{
		new SiegeGuards();
	}
}
