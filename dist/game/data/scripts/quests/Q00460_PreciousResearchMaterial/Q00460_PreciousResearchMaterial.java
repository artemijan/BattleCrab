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
package quests.Q00460_PreciousResearchMaterial;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;
import org.l2jmobius.gameserver.model.script.State;

/**
 * Precious Research Material (460)
 * @URL https://l2wiki.com/Precious_Research_Material
 * @author Gigi
 */
public class Q00460_PreciousResearchMaterial extends Quest
{
	// NPCs
	private static final int AMER = 33092;
	private static final int FILAUR = 30535;
	
	// Monster
	private static final int EGG = 18997;
	
	// Item's
	private static final int PROOF_OF_FIDELITY = 19450; //
	private static final int TEREDOR_EGG_FRAGMENT = 17735;
	
	// Misc
	private static final int MIN_LEVEL = 85;
	
	public Q00460_PreciousResearchMaterial()
	{
		super(460);
		addStartNpc(AMER);
		addTalkId(AMER, FILAUR);
		addKillId(EGG);
		registerQuestItems(TEREDOR_EGG_FRAGMENT);
		addCondMinLevel(MIN_LEVEL, "30535-00.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "30535-02.html":
			{
				giveItems(player, PROOF_OF_FIDELITY, 3);
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
		switch (npc.getId())
		{
			case AMER:
			{
				switch (qs.getState())
				{
					case State.COMPLETED:
					{
						if (!qs.isNowAvailable())
						{
							htmltext = getNoQuestMsg(player);
							break;
						}
						
						qs.setState(State.CREATED);
						// fallthrough
					}
					case State.CREATED:
					{
						htmltext = "33092-02.html";
						qs.startQuest();
						break;
					}
					case State.STARTED:
					{
						htmltext = "33092-01.htm";
						break;
					}
				}
				break;
			}
			case FILAUR:
			{
				if (qs.isCond(2) && (getQuestItemsCount(player, TEREDOR_EGG_FRAGMENT) >= 20))
				{
					htmltext = "30535-01.html";
				}
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getRandomPartyMemberState(killer, 1, 3, npc);
		if ((qs != null) && giveItemRandomly(killer, TEREDOR_EGG_FRAGMENT, 1, 20, 0.7, true))
		{
			qs.setCond(2, true);
		}
	}
}
