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
package quests.Q00197_SevenSignsTheSacredBookOfSeal;

import org.l2jmobius.gameserver.ai.Intention;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.instance.Monster;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestSound;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;
import org.l2jmobius.gameserver.network.NpcStringId;
import org.l2jmobius.gameserver.network.enums.ChatType;

import quests.Q00196_SevenSignsSealOfTheEmperor.Q00196_SevenSignsSealOfTheEmperor;

/**
 * Seven Signs, The Sacred Book of Seal (197)
 * @author Adry_85
 */
public class Q00197_SevenSignsTheSacredBookOfSeal extends Quest
{
	// NPCs
	private static final int SHILENS_EVIL_THOUGHTS = 27396;
	private static final int ORVEN = 30857;
	private static final int WOOD = 32593;
	private static final int LEOPARD = 32594;
	private static final int LAWRENCE = 32595;
	private static final int SOPHIA = 32596;
	
	// Items
	private static final int MYSTERIOUS_HAND_WRITTEN_TEXT = 13829;
	private static final int SCULPTURE_OF_DOUBT = 14354;
	
	// Misc
	private static final int MIN_LEVEL = 79;
	private boolean isBusy = false;
	
	public Q00197_SevenSignsTheSacredBookOfSeal()
	{
		super(197);
		addStartNpc(WOOD);
		addTalkId(WOOD, ORVEN, LEOPARD, LAWRENCE, SOPHIA);
		addKillId(SHILENS_EVIL_THOUGHTS);
		addCondMinLevel(MIN_LEVEL, "32593-05.html");
		addCondCompletedQuest(Q00196_SevenSignsSealOfTheEmperor.class.getSimpleName(), "32593-05.html");
		registerQuestItems(MYSTERIOUS_HAND_WRITTEN_TEXT, SCULPTURE_OF_DOUBT);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		if ((npc.getId() == SHILENS_EVIL_THOUGHTS) && "despawn".equals(event))
		{
			if (!npc.isDead())
			{
				isBusy = false;
				npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.NEXT_TIME_YOU_WILL_NOT_ESCAPE);
				npc.deleteMe();
			}
			
			return super.onEvent(event, npc, player);
		}
		
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return null;
		}
		
		String htmltext = null;
		switch (event)
		{
			case "32593-02.htm":
			case "32593-03.htm":
			{
				htmltext = event;
				break;
			}
			case "32593-04.html":
			{
				qs.startQuest();
				htmltext = event;
				break;
			}
			case "32593-08.html":
			{
				if (qs.isCond(6) && hasQuestItems(player, MYSTERIOUS_HAND_WRITTEN_TEXT, SCULPTURE_OF_DOUBT))
				{
					htmltext = event;
				}
				break;
			}
			case "32593-09.html":
			{
				if (qs.isCond(6))
				{
					if (player.getLevel() >= MIN_LEVEL)
					{
						addExpAndSp(player, 52518015, 5817677);
						qs.exitQuest(false, true);
						htmltext = event;
					}
					else
					{
						htmltext = "level_check.html";
					}
				}
				break;
			}
			case "30857-02.html":
			case "30857-03.html":
			{
				if (qs.isCond(1))
				{
					htmltext = event;
				}
				break;
			}
			case "30857-04.html":
			{
				if (qs.isCond(1))
				{
					qs.setCond(2, true);
					htmltext = event;
				}
				break;
			}
			case "32594-02.html":
			{
				if (qs.isCond(2))
				{
					htmltext = event;
				}
				break;
			}
			case "32594-03.html":
			{
				if (qs.isCond(2))
				{
					qs.setCond(3, true);
					htmltext = event;
				}
				break;
			}
			case "32595-02.html":
			case "32595-03.html":
			{
				if (qs.isCond(3))
				{
					htmltext = event;
				}
				break;
			}
			case "32595-04.html":
			{
				if (qs.isCond(3))
				{
					isBusy = true;
					npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.S1_THAT_STRANGER_MUST_BE_DEFEATED_HERE_IS_THE_ULTIMATE_HELP, player.getName());
					final Monster monster = addSpawn(SHILENS_EVIL_THOUGHTS, 152520, -57502, -3408, 0, false, 0, false).asMonster();
					monster.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.YOU_ARE_NOT_THE_OWNER_OF_THAT_ITEM);
					monster.setRunning();
					monster.addDamageHate(player, 0, 999);
					monster.getAI().setIntention(Intention.ATTACK, player);
					startQuestTimer("despawn", 300000, monster, null);
				}
				break;
			}
			case "32595-06.html":
			case "32595-07.html":
			case "32595-08.html":
			{
				if (qs.isCond(4) && hasQuestItems(player, SCULPTURE_OF_DOUBT))
				{
					htmltext = event;
				}
				break;
			}
			case "32595-09.html":
			{
				if (qs.isCond(4) && hasQuestItems(player, SCULPTURE_OF_DOUBT))
				{
					qs.setCond(5, true);
					htmltext = event;
				}
				break;
			}
			case "32596-02.html":
			case "32596-03.html":
			{
				if (qs.isCond(5) && hasQuestItems(player, SCULPTURE_OF_DOUBT))
				{
					htmltext = event;
				}
				break;
			}
			case "32596-04.html":
			{
				if (qs.isCond(5) && hasQuestItems(player, SCULPTURE_OF_DOUBT))
				{
					giveItems(player, MYSTERIOUS_HAND_WRITTEN_TEXT, 1);
					qs.setCond(6, true);
					htmltext = event;
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player player, boolean isSummon)
	{
		final Player partyMember = getRandomPartyMember(player, 3);
		if (partyMember == null)
		{
			return;
		}
		
		final QuestState qs = getQuestState(partyMember, false);
		if (npc.isInsideRadius3D(partyMember, PlayerConfig.ALT_PARTY_RANGE))
		{
			giveItems(partyMember, SCULPTURE_OF_DOUBT, 1);
			playSound(partyMember, QuestSound.ITEMSOUND_QUEST_FINISH);
			qs.setCond(4);
		}
		
		isBusy = false;
		cancelQuestTimers("despawn");
		npc.broadcastSay(ChatType.NPC_GENERAL, NpcStringId.S1_YOU_MAY_HAVE_WON_THIS_TIME_BUT_NEXT_TIME_I_WILL_SURELY_CAPTURE_YOU, partyMember.getName());
	}
	
	@Override
	public String onTalk(Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, true);
		String htmltext = getNoQuestMsg(player);
		switch (qs.getState())
		{
			case State.COMPLETED:
			{
				htmltext = getAlreadyCompletedMsg(player);
				break;
			}
			case State.CREATED:
			{
				if (npc.getId() == WOOD)
				{
					htmltext = "32593-01.htm";
				}
				break;
			}
			case State.STARTED:
			{
				switch (npc.getId())
				{
					case WOOD:
					{
						if ((qs.getCond() > 0) && (qs.getCond() < 6))
						{
							htmltext = "32593-06.html";
						}
						else if (qs.isCond(6))
						{
							if (hasQuestItems(player, MYSTERIOUS_HAND_WRITTEN_TEXT, SCULPTURE_OF_DOUBT))
							{
								htmltext = "32593-07.html";
							}
						}
						break;
					}
					case ORVEN:
					{
						if (qs.isCond(1))
						{
							htmltext = "30857-01.html";
						}
						else if (qs.getCond() >= 2)
						{
							htmltext = "30857-05.html";
						}
						break;
					}
					case LEOPARD:
					{
						if (qs.isCond(2))
						{
							htmltext = "32594-01.html";
						}
						else if (qs.getCond() >= 3)
						{
							htmltext = "32594-04.html";
						}
						break;
					}
					case LAWRENCE:
					{
						if (qs.isCond(3))
						{
							if (isBusy)
							{
								htmltext = "32595-05.html";
							}
							else
							{
								htmltext = "32595-01.html";
							}
						}
						else if (qs.isCond(4))
						{
							if (hasQuestItems(player, SCULPTURE_OF_DOUBT))
							{
								htmltext = "32595-06.html";
							}
						}
						else if (qs.getCond() >= 5)
						{
							if (hasQuestItems(player, SCULPTURE_OF_DOUBT))
							{
								htmltext = "32595-10.html";
							}
						}
						break;
					}
					case SOPHIA:
					{
						if (qs.isCond(5))
						{
							if (hasQuestItems(player, SCULPTURE_OF_DOUBT))
							{
								htmltext = "32596-01.html";
							}
						}
						else if (qs.getCond() >= 6)
						{
							if (hasQuestItems(player, SCULPTURE_OF_DOUBT, MYSTERIOUS_HAND_WRITTEN_TEXT))
							{
								htmltext = "32596-05.html";
							}
						}
						break;
					}
				}
				break;
			}
		}
		
		return htmltext;
	}
}
