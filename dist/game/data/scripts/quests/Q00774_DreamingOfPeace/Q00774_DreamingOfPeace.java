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
package quests.Q00774_DreamingOfPeace;

import java.util.HashSet;
import java.util.Set;

import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.NpcLogListHolder;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;

/**
 * Dreaming of Peace (774)
 * @URL https://l2wiki.com/Dreaming_of_Peace
 * @author Dmitri
 */
public class Q00774_DreamingOfPeace extends Quest
{
	// NPC
	private static final int NERUPA = 30370;
	
	// Monsters
	private static final int[] MONSTERS =
	{
		22867, // Fairy Warrior-Violent
		22868, // Fairy Warrior-Brutal
		22869, // Fairy Warrior-Fully Enraged
		22870, // Fairy Warrior-Slightly Enraged
		22875, // Fairy Rogue-Violent
		22876, // Fairy Rogue-Brutal
		22877, // Fairy Rogue-Fully Enraged
		22878, // Fairy Rogue-Slightly Enraged
		22883, // Fairy Knight-Violent
		22884, // Fairy Knight-Brutal
		22885, // Fairy Knight-Fully Enraged
		22886, // Fairy Knight-Slightly Enraged
		22891, // Satyr Wizard-Violent
		22892, // Satyr Wizard-Brutal
		22893, // Satyr Wizard-Fully Enraged
		22894, // Satyr Wizard-Slightly Enraged
		22899, // Satyr Summoner-Violent
		22900, // Satyr Summoner-Brutal
		22901, // Satyr Summoner-Fully Enraged
		22902, // Satyr Summoner-Slightly Enraged
		22907, // Satyr Witch-Violent
		22908, // Satyr Witch-Brutal
		22909, // Satyr Witch-Fully Enraged
		22910, // Satyr Witch-Slightly Enraged
		23763, // Fairy Knight-Enraged
		23765, // Fairy Rogue-Enraged
		23767, // Fairy Warrior-Enraged
		23769, // Satyr Summoner-Enraged
		23771, // Satyr Witch-Enraged
		23773, // Satyr Wizard-Enraged
	};
	
	// Misc
	private static final int MIN_LEVEL = 90;
	private static final int MAX_LEVEL = 100;
	private static final String KILL_COUNT_VAR = "KillCount";
	
	public Q00774_DreamingOfPeace()
	{
		super(774);
		addStartNpc(NERUPA);
		addTalkId(NERUPA);
		addKillId(MONSTERS);
		addCondLevel(MIN_LEVEL, MAX_LEVEL, "30370-00.htm");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return htmltext;
		}
		
		switch (event)
		{
			case "30370-02.htm":
			case "30370-03.htm":
			{
				htmltext = event;
				break;
			}
			case "30370-04.htm":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "30370-07.html":
			{
				// Rewards
				giveAdena(player, 559020, true);
				addExpAndSp(player, 646727130, 646710);
				qs.exitQuest(QuestType.DAILY, true);
				htmltext = event;
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "30370-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = (qs.isCond(1)) ? "30370-05.html" : "30370-06.html";
				break;
			}
			case State.COMPLETED:
			{
				if (qs.isNowAvailable())
				{
					qs.setState(State.CREATED);
					htmltext = "30370-01.htm";
				}
				else
				{
					htmltext = getAlreadyCompletedMsg(player, QuestType.DAILY);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1) && killer.isInsideRadius3D(npc, PlayerConfig.ALT_PARTY_RANGE))
		{
			int count = qs.getInt(KILL_COUNT_VAR);
			qs.set(KILL_COUNT_VAR, ++count);
			if (count >= 300)
			{
				qs.setCond(2, true);
			}
			else
			{
				playSound(killer, QuestSound.ITEMSOUND_QUEST_ITEMGET);
			}
		}
	}
	
	@Override
	public Set<NpcLogListHolder> getNpcLogList(Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if ((qs != null) && qs.isCond(1))
		{
			final int killCount = qs.getInt(KILL_COUNT_VAR);
			if (killCount > 0)
			{
				final Set<NpcLogListHolder> holder = new HashSet<>();
				holder.add(new NpcLogListHolder(NpcStringId.DEFEAT_THE_VIOLENT_MONSTERS.getId(), true, killCount));
				return holder;
			}
		}
		
		return super.getNpcLogList(player);
	}
}
