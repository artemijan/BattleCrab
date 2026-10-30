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
package instances.JiniaGuildHideout1;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.instancezone.Instance;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;

import quests.Q10284_AcquisitionOfDivineSword.Q10284_AcquisitionOfDivineSword;

/**
 * Jinia Guild Hideout instance zone.
 * @author Adry_85
 */
public class JiniaGuildHideout1 extends InstanceScript
{
	// NPC
	private static final int RAFFORTY = 32020;
	
	// Misc
	private static final int TEMPLATE_ID = 140;
	
	public JiniaGuildHideout1()
	{
		super(TEMPLATE_ID);
		addStartNpc(RAFFORTY);
		addTalkId(RAFFORTY);
	}
	
	@Override
	protected void onEnter(Player player, Instance instance, boolean firstEnter)
	{
		super.onEnter(player, instance, firstEnter);
		if (firstEnter)
		{
			final QuestState qs = player.getQuestState(Q10284_AcquisitionOfDivineSword.class.getSimpleName());
			if (qs != null)
			{
				qs.setCond(2, true);
			}
		}
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		enterInstance(talker, npc, TEMPLATE_ID);
		return super.onTalk(npc, talker);
	}
	
	public static void main(String[] args)
	{
		new JiniaGuildHideout1();
	}
}
