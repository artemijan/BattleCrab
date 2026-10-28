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
package quests.Q10453_StoppingTheWindDragon;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.QuestType;

/**
 * @author hlwrave
 */
public class Q10453_StoppingTheWindDragon extends Quest
{
	// NPC
	private static final int JENNA = 33872;
	
	// Monsters
	private static final int LINDVIOR = 29240;
	
	// Items
	private static final int LINDVIOR_SLAYERS_HELMET = 37497;
	
	// Misc
	private static final int MIN_LEVEL = 95;
	
	public Q10453_StoppingTheWindDragon()
	{
		super(10453);
		addStartNpc(JENNA);
		addTalkId(JENNA);
		addKillId(LINDVIOR);
		addCondMinLevel(MIN_LEVEL, "adens_wizard_jenna_q10453_0.html");
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final String htmltext = event;
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "adens_wizard_jenna_q10453_2.html":
			{
				qs.startQuest();
				break;
			}
			case "adens_wizard_jenna_q10453_5.html":
			{
				addExpAndSp(player, 2147483500, 37047780);
				giveItems(player, LINDVIOR_SLAYERS_HELMET, 1);
				qs.exitQuest(QuestType.ONE_TIME, true);
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
		if (qs.isCreated())
		{
			htmltext = "adens_wizard_jenna_q10453_1.htm";
		}
		else if (qs.isStarted())
		{
			if (qs.isCond(1))
			{
				htmltext = "adens_wizard_jenna_q10453_3.html";
			}
			else if (qs.isCond(2))
			{
				htmltext = "adens_wizard_jenna_q10453_4.html";
			}
		}
		else if (qs.isCompleted())
		{
			htmltext = "adens_wizard_jenna_q10453_6.html";
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			qs.setCond(2);
		}
	}
}
