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
package instances.SSQElcadiasTent;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.InstanceScript;
import org.l2jmobius.gameserver.model.script.QuestState;

import quests.Q10292_SevenSignsGirlOfDoubt.Q10292_SevenSignsGirlOfDoubt;
import quests.Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom.Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom;
import quests.Q10294_SevenSignsToTheMonasteryOfSilence.Q10294_SevenSignsToTheMonasteryOfSilence;
import quests.Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal.Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal;

/**
 * Elcadia's Tent instance zone.
 * @author Adry_85
 */
public class SSQElcadiasTent extends InstanceScript
{
	// NPCs
	private static final int ELCADIA = 32784;
	private static final int GRUFF_LOOKING_MAN = 32862;
	
	// Misc
	private static final int TEMPLATE_ID = 158;
	
	public SSQElcadiasTent()
	{
		super(TEMPLATE_ID);
		addFirstTalkId(GRUFF_LOOKING_MAN, ELCADIA);
		addStartNpc(GRUFF_LOOKING_MAN, ELCADIA);
		addTalkId(GRUFF_LOOKING_MAN, ELCADIA);
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		if (npc.getId() == GRUFF_LOOKING_MAN)
		{
			final QuestState girlOfDoubt = talker.getQuestState(Q10292_SevenSignsGirlOfDoubt.class.getSimpleName());
			final QuestState forbiddenBook = talker.getQuestState(Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom.class.getSimpleName());
			final QuestState monastery = talker.getQuestState(Q10294_SevenSignsToTheMonasteryOfSilence.class.getSimpleName());
			final QuestState powerOfSeal = talker.getQuestState(Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal.class.getSimpleName());
			if (((girlOfDoubt != null) && girlOfDoubt.isStarted()) //
				|| ((girlOfDoubt != null) && girlOfDoubt.isCompleted() && (forbiddenBook == null)) //
				|| ((forbiddenBook != null) && forbiddenBook.isStarted()) //
				|| ((forbiddenBook != null) && forbiddenBook.isCompleted() && (monastery == null)) //
				|| ((powerOfSeal != null) && powerOfSeal.isStarted()))
			{
				enterInstance(talker, npc, TEMPLATE_ID);
			}
			else
			{
				return "32862-01.html";
			}
		}
		else
		{
			finishInstance(talker, 0);
		}
		
		return super.onTalk(npc, talker);
	}
	
	public static void main(String[] args)
	{
		new SSQElcadiasTent();
	}
}
