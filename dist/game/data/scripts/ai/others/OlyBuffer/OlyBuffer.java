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
package ai.others.OlyBuffer;

import org.l2jmobius.gameserver.config.OlympiadConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.model.skill.SkillCaster;
import org.l2jmobius.gameserver.model.skill.holders.SkillHolder;

/**
 * Olympiad Buffer AI.
 * @author St3eT, Mobius
 */
public class OlyBuffer extends Script
{
	// NPC
	private static final int OLYMPIAD_BUFFER = 36402;
	
	// Skills
	private static final SkillHolder KNIGHT = new SkillHolder(14744, 1); // Olympiad - Knight's Harmony
	private static final SkillHolder WARRIOR = new SkillHolder(14745, 1); // Olympiad - Warrior's Harmony
	private static final SkillHolder WIZARD = new SkillHolder(14746, 1); // Olympiad - Wizard's Harmony
	private static final SkillHolder[] BUFFS =
	{
		new SkillHolder(14738, 1), // Olympiad - Horn Melody
		new SkillHolder(14739, 1), // Olympiad - Drum Melody
		new SkillHolder(14740, 1), // Olympiad - Pipe Organ Melody
		new SkillHolder(14741, 1), // Olympiad - Guitar Melody
	};
	
	private OlyBuffer()
	{
		if (OlympiadConfig.OLYMPIAD_ENABLED)
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
		if (npc.isScriptValue(0))
		{
			htmltext = "olympiad_master001.htm";
		}
		
		return htmltext;
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		switch (event)
		{
			case "guardian":
			{
				applyBuffs(npc, player, KNIGHT);
				break;
			}
			case "berserker":
			{
				applyBuffs(npc, player, WARRIOR);
				break;
			}
			case "magician":
			{
				applyBuffs(npc, player, WIZARD);
				break;
			}
		}
		
		npc.setScriptValue(1);
		getTimers().addTimer("DELETE_ME", 5000, evnt -> npc.deleteMe());
		return "olympiad_master003.htm";
	}
	
	private void applyBuffs(Npc npc, Player player, SkillHolder skill)
	{
		for (SkillHolder holder : BUFFS)
		{
			SkillCaster.triggerCast(npc, player, holder.getSkill());
		}
		
		SkillCaster.triggerCast(npc, player, skill.getSkill());
	}
	
	public static void main(String[] args)
	{
		new OlyBuffer();
	}
}
