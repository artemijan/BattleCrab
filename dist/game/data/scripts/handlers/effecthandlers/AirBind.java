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

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

import org.l2jmobius.gameserver.ai.CtrlEvent;
import org.l2jmobius.gameserver.model.StatSet;
import org.l2jmobius.gameserver.model.actor.Creature;
import org.l2jmobius.gameserver.model.effects.AbstractEffect;
import org.l2jmobius.gameserver.model.item.instance.Item;
import org.l2jmobius.gameserver.model.skill.Skill;

/**
 * @author Mobius
 */
public class AirBind extends AbstractEffect
{
	private static final Set<Creature> ACTIVE_AIRBINDS = ConcurrentHashMap.newKeySet();
	// private static final Map<ClassId, Integer> AIRBIND_SKILLS = new EnumMap<>(ClassId.class);
	// static
	// {
	// AIRBIND_SKILLS.put(ClassId.SIGEL_PHOENIX_KNIGHT, 10249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.SIGEL_HELL_KNIGHT, 10249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.SIGEL_EVA_TEMPLAR, 10249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.SIGEL_SHILLIEN_TEMPLAR, 10249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.TYRR_DUELIST, 10499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.TYRR_DREADNOUGHT, 10499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.TYRR_TITAN, 10499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.TYRR_GRAND_KHAVATARI, 10499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.TYRR_MAESTRO, 10499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.TYRR_DOOMBRINGER, 10499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.OTHELL_ADVENTURER, 10749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.OTHELL_WIND_RIDER, 10749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.OTHELL_GHOST_HUNTER, 10749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.OTHELL_FORTUNE_SEEKER, 10749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.YUL_SAGITTARIUS, 10999); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.YUL_MOONLIGHT_SENTINEL, 10999); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.YUL_GHOST_SENTINEL, 10999); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.YUL_TRICKSTER, 10999); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.FEOH_ARCHMAGE, 11249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.FEOH_SOULTAKER, 11249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.FEOH_MYSTIC_MUSE, 11249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.FEOH_STORM_SCREAMER, 11249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.FEOH_SOUL_HOUND, 11249); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.ISS_HIEROPHANT, 11749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.ISS_SWORD_MUSE, 11749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.ISS_SPECTRAL_DANCER, 11749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.ISS_DOMINATOR, 11749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.ISS_DOOMCRYER, 11749); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.WYNN_ARCANA_LORD, 11499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.WYNN_ELEMENTAL_MASTER, 11499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.WYNN_SPECTRAL_MASTER, 11499); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.AEORE_CARDINAL, 11999); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.AEORE_EVA_SAINT, 11999); // Heavy Hit
	// AIRBIND_SKILLS.put(ClassId.AEORE_SHILLIEN_SAINT, 11999); // Heavy Hit
	// }
	
	public AirBind(StatSet params)
	{
	}
	
	@Override
	public boolean isInstant()
	{
		return false;
	}
	
	@Override
	public void continuousInstant(Creature effector, Creature effected, Skill skill, Item item)
	{
        // for (Player nearbyPlayer : World.getInstance().getVisibleObjectsInRange(effected, Player.class, 1200))
        // {
        // if ((nearbyPlayer.getRace() != Race.ERTHEIA) && (nearbyPlayer.getTarget() == effected) && nearbyPlayer.isInCategory(CategoryType.SIXTH_CLASS_GROUP) && !nearbyPlayer.isAlterSkillActive())
        // {
        // final int chainSkill = AIRBIND_SKILLS.get(nearbyPlayer.getClassId());
        // if (nearbyPlayer.getSkillRemainingReuseTime(chainSkill) == -1)
        // {
        // nearbyPlayer.sendPacket(new ExAlterSkillRequest(nearbyPlayer, chainSkill, chainSkill, 5));
        // }
        // }
        // }
        ACTIVE_AIRBINDS.add(effected);
	}
	
	@Override
	public void onExit(Creature effector, Creature effected, Skill skill)
	{
		ACTIVE_AIRBINDS.remove(effected);
		
		if (!effected.isPlayer())
		{
			effected.getAI().notifyEvent(CtrlEvent.EVT_THINK);
		}
	}
}
