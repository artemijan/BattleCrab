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
package ai.others.OlyBuffer;

import org.l2jmobius.Config;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.holders.SkillHolder;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.network.serverpackets.MagicSkillUse;

import ai.AbstractNpcAI;

/**
 * Olympiad Buffer AI.
 * @author St3eT
 */
public class OlyBuffer extends AbstractNpcAI
{
	// NPC
	private static final int OLYMPIAD_BUFFER = 36402;
	// Skills
	private static final SkillHolder[] ALLOWED_BUFFS =
	{
		new SkillHolder(1086, 1), // Haste - Atk. Spd. +15%.
		new SkillHolder(1085, 1), // Acumen - Casting Spd. +15%
		new SkillHolder(1204, 1), // Wind Walk - Speed +20.
		new SkillHolder(1068, 1), // Might - P. Atk. +8%.
		new SkillHolder(1040, 1), // Shield - P. Def. +8%.
		new SkillHolder(1036, 1), // Magic Barrier - M. Def. +23%.
		new SkillHolder(1045, 1), // Blessed Body - Max HP +10%.
		new SkillHolder(1048, 1), // Blessed Soul - Max MP +10%.
		new SkillHolder(1062, 1), // Berserker Spirit - P. Def. -5%, M. Def. -10%, P. Evasion -2, P. Atk. +5%, M. Atk. +10%, Atk. Spd. +5%, Casting Spd. +5% and Speed +5.
	};
	
	private OlyBuffer()
	{
		if (Config.OLYMPIAD_ENABLED)
		{
			addStartNpc(OLYMPIAD_BUFFER);
			addFirstTalkId(OLYMPIAD_BUFFER);
			addTalkId(OLYMPIAD_BUFFER);
		}
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = null;
		if (npc.getScriptValue() < 5)
		{
			htmltext = "OlyBuffer-index.html";
		}
		return htmltext;
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		if (event.startsWith("giveBuff;") && (npc.getScriptValue() < 5))
		{
			final int buffId = Integer.parseInt(event.replace("giveBuff;", ""));
			if (ALLOWED_BUFFS[buffId] != null)
			{
				npc.setScriptValue(npc.getScriptValue() + 1);
				final Skill skill = ALLOWED_BUFFS[buffId].getSkill();
				player.sendPacket(new MagicSkillUse(npc, player, skill.getDisplayId(), skill.getLevel(), 0, 0));
				skill.applyEffects(npc, player);
				htmltext = "OlyBuffer-afterBuff.html";
			}
			if (npc.getScriptValue() >= 5)
			{
				htmltext = "OlyBuffer-noMore.html";
				getTimers().addTimer("DELETE_ME", 5000, evnt -> npc.deleteMe());
			}
		}
		return htmltext;
	}
	
	public static void main(String[] args)
	{
		new OlyBuffer();
	}
}
