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
package ai.areas.GuillotineFortress;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.Skill;
import org.l2jmobius.gameserver.model.skill.enums.SkillFinishType;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.serverpackets.ExShowScreenMessage;

/**
 * Guillotine Fortress AI.
 * @author Gigi
 */
public class GuillotineFortress extends Script
{
	private static final int[] MONSTERS =
	{
		23199, // Sadiac the Killer
		23200, // Turan the Severed Ghost
		23201, // Nagdu the Deformed Merman
		23202, // Hakal the Butchered
		23203, // Centa the Standing Beast
		23204, // Adidaiu of the Killer Fangs
		23205, // Haskal the Floating Ghost
		23206, // Samita the Ex-Torture Expert
		23207, // Gazem
		23208, // Rosenia's Divine Spirit
		23209, // Kelbara the Dualsword Slaughterer
		23212, // Scaldisect the Furious
		23242, // Pafuron
		23243, // Krutati
		23244, // Alluring Irene
		23245 // Isaad
	};
	private static final SkillHolder CHAOS_SHIELD = new SkillHolder(15208, 9);
	private static final int PROOF_OF_SURVIVAL = 34898;
	private static final int SCALDISECT_THE_FURIOUS = 23212;
	
	public GuillotineFortress()
	{
		addAttackId(MONSTERS);
		addKillId(MONSTERS);
	}
	
	@Override
	public void onAttack(Npc npc, Player player, int damage, boolean isPet, Skill skill)
	{
		if ((npc.getCurrentHpPercent() <= 85) && npc.isScriptValue(1))
		{
			npc.getEffectList().stopSkillEffects(SkillFinishType.REMOVED, CHAOS_SHIELD.getSkillId());
			final Party party = player.getParty();
			if (party == null)
			{
				player.broadcastPacket(new ExShowScreenMessage(NpcStringId.CHAOS_SHIELD_BREAKTHROUGH, ExShowScreenMessage.BOTTOM_CENTER, 10000, false));
			}
			else
			{
				for (Player mem : party.getMembers())
				{
					mem.broadcastPacket(new ExShowScreenMessage(NpcStringId.CHAOS_SHIELD_BREAKTHROUGH, ExShowScreenMessage.BOTTOM_CENTER, 10000, false));
				}
			}
		}
		else if ((npc.getCurrentHpPercent() > 85) && npc.isScriptValue(0))
		{
			addSkillCastDesire(npc, npc, CHAOS_SHIELD, 23);
			npc.setScriptValue(1);
		}
		
		if ((player.getInventory().getItemByItemId(PROOF_OF_SURVIVAL) != null) && (getRandom(100) < 1))
		{
			addSpawn(SCALDISECT_THE_FURIOUS, player.getX(), player.getY(), player.getZ(), 0, true, 120000);
			takeItems(player, PROOF_OF_SURVIVAL, 1);
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		if ((killer != null) && killer.isPlayer() && (getRandom(100) < 5))
		{
			giveItems(killer, PROOF_OF_SURVIVAL, 1);
		}
	}
	
	public static void main(String[] args)
	{
		new GuillotineFortress();
	}
}
