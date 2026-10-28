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
package ai.others.MysteriousWizard;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.Script;

import quests.Q10751_WindsOfFateEncounters.Q10751_WindsOfFateEncounters;

/**
 * Mysterious Wizard AI.
 * @author Gladicek
 */
public class MysteriousWizard extends Script
{
	// Npc
	private static final int MYSTERIOUS_WIZARD = 33980;
	
	// Misc
	private static final int FORTRESS_OF_THE_DEAD = 254;
	
	private MysteriousWizard()
	{
		addFirstTalkId(MYSTERIOUS_WIZARD);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		return event.equals("33980-01.html") ? event : null;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = player.getQuestState(Q10751_WindsOfFateEncounters.class.getSimpleName());
		final Instance world = npc.getInstanceWorld();
		if (isFotDInstance(world))
		{
			htmltext = "33980.html";
		}
		else if (qs != null)
		{
			if (qs.isCond(6))
			{
				htmltext = "33980-05.html";
			}
			else if (qs.isCond(7))
			{
				htmltext = "33980-04.html";
			}
		}
		
		return htmltext;
	}
	
	private boolean isFotDInstance(Instance instance)
	{
		return (instance != null) && (instance.getTemplateId() == FORTRESS_OF_THE_DEAD);
	}
	
	public static void main(String[] args)
	{
		new MysteriousWizard();
	}
}
