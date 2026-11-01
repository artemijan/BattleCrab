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
package quests.Q00663_SeductiveWhispers;

import java.util.Arrays;
import java.util.stream.Collectors;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Quest;
import org.l2jmobius.gameserver.model.script.QuestState;
import org.l2jmobius.gameserver.model.script.State;

/**
 * @author Mathael
 */
public class Q00663_SeductiveWhispers extends Quest
{
	// NPC
	private static final int WILBERT = 30846;
	
	// Item
	private static final int SPIRIT_BEAD = 8766;
	
	// Monsters
	private static final int[] MOBS =
	{
		20956,
		20955,
		20954,
		20959,
		20958,
		20957,
		20963,
		20962,
		20961,
		20960,
		20976,
		20974,
		20975,
		21006,
		20674,
		21010,
		21009,
		21001,
		21002
	};
	
	// Rewards
	private static final int[] RECIPES_B_GRADE =
	{
		5000,
		4966,
		4967,
		4963,
		4968,
		5005,
		4969,
		5007,
		4971,
		5008,
		4970,
		5006,
		5001,
		4972,
		4964,
		5002,
		5003,
		4965,
		5004
	};
	private static final int[] PART_B_GRADE =
	{
		4114,
		4107,
		4108,
		1104,
		4109,
		4119,
		4110,
		4121,
		4112,
		2120,
		4111,
		4120,
		4115,
		4113,
		4105,
		4116,
		4117,
		4106,
		4118
	};
	
	// Misc
	private static final Card[] cards = new Card[10];
	static
	{
		cards[0] = new Card(Side.MOON, 1);
		cards[1] = new Card(Side.SUN, 1);
		cards[2] = new Card(Side.MOON, 2);
		cards[3] = new Card(Side.SUN, 2);
		cards[4] = new Card(Side.MOON, 3);
		cards[5] = new Card(Side.SUN, 3);
		cards[6] = new Card(Side.MOON, 4);
		cards[7] = new Card(Side.SUN, 4);
		cards[8] = new Card(Side.MOON, 5);
		cards[9] = new Card(Side.SUN, 5);
	}
	private int winCount = 0;
	private Card playerCard = null;
	private Card npcCard = null;
	
	public Q00663_SeductiveWhispers()
	{
		super(663);
		addStartNpc(WILBERT);
		addTalkId(WILBERT);
		addKillId(MOBS);
		addCondMinLevel(50, "30846-lvl.html");
	}
	
	@Override
	public String onTalk(Npc npc, Player talker)
	{
		final QuestState qs = getQuestState(talker, true);
		String htmltext = getNoQuestMsg(talker);
		
		switch (qs.getState())
		{
			case State.CREATED:
			{
				htmltext = "30846-01.htm";
				break;
			}
			case State.STARTED:
			{
				htmltext = "30846-09.html";
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		final QuestState qs = getQuestState(player, false);
		if (qs == null)
		{
			return getNoQuestMsg(player);
		}
		
		switch (event)
		{
			case "30846-02.htm":
			case "30846-04.htm":
			case "30846-05.html":
			case "30846-11.html":
			case "30846-12.html":
			case "30846-14.html":
			case "30846-19.html":
			{
				break;
			}
			case "30846-03.htm":
			{
				qs.startQuest();
				break;
			}
			case "30846-10.html":
			{
				qs.exitQuest(true, true);
				break;
			}
			case "30846-06.html":
			{
				// practice
				if (getQuestItemsCount(player, SPIRIT_BEAD) < 1)
				{
					return "30846-13.html";
				}
				
				takeItems(player, SPIRIT_BEAD, 1);
				break;
			}
			case "30846-22.html":
			{
				if (playerCard == null)
				{
					playerCard = cards[getRandom(0, 9)];
				}
				else
				{
					playerCard = getRandomCard();
				}
				
				return play(player, true, true);
			}
			case "30846-23.html":
			{
				if (npcCard == null)
				{
					npcCard = playerCard; // to avoid same card when getRandomCard()
					npcCard = getRandomCard();
				}
				else
				{
					npcCard = getRandomCard();
				}
				
				return play(player, false, true);
			}
			case "30846-15.html":
			{
				if (getQuestItemsCount(player, SPIRIT_BEAD) < 50)
				{
					return "30846-08.html";
				}
				
				takeItems(player, SPIRIT_BEAD, 50);
				break;
			}
			case "30846-16.html":
			{
				// Player draw a card (Always player start first !!)
				if (playerCard == null)
				{
					playerCard = cards[getRandom(0, 9)];
				}
				else
				{
					playerCard = getRandomCard();
				}
				
				return play(player, true, false);
			}
			case "30846-17.html":
			{
				// npc draw card
				if (npcCard == null)
				{
					npcCard = playerCard; // to avoid same card when getRandomCard()
					npcCard = getRandomCard();
				}
				else
				{
					npcCard = getRandomCard();
				}
				
				return play(player, false, false);
			}
			case "30846-20.html":
			{
				if (winCount > 0)
				{
					giveRewards(player, winCount);
					winCount = 0;
				}
				
				reset(false);
				break;
			}
			case "30846-21.html":
			{
				reset(true);
				break;
			}
		}
		
		return event;
	}
	
	private String play(Player player, boolean isPlayerTurn, boolean isPractice)
	{
		boolean isPlayerWin = false;
		boolean isNpcWin = false;
		String htmltext;
		final String table = getHtm(player, "table.html");
		if (isPlayerTurn)
		{
			if ((playerCard != null) && isWinner(playerCard, npcCard, true))
			{
				if (isPractice)
				{
					rewardItems(player, 57, 800);
				}
				else
				{
					winCount++;
				}
				
				if (winCount == 8)
				{
					// Cannot win more than 8 consecutive game (I don't know html after 8 wins (too hard) -> give reward and show main page)
					giveRewards(player, winCount);
					reset(true);
					return "30846-05.html";
				}
				
				isPlayerWin = true;
				htmltext = getHtm(player, isPractice ? "30846-07.html" : "30846-18.html");
				htmltext = htmltext.replace("%table%", table);
				htmltext = htmltext.replace("%win_count%", Integer.toString(winCount));
			}
			else
			{
				htmltext = getHtm(player, isPractice ? "30846-22.html" : "30846-16.html");
				htmltext = htmltext.replace("%table%", table);
			}
		}
		else if ((playerCard != null) && (npcCard != null) && isWinner(playerCard, npcCard, false))
		{
			htmltext = getHtm(player, isPractice ? "30846-24.html" : "30846-21.html");
			htmltext = htmltext.replace("%table%", table);
			isNpcWin = true;
		}
		else
		{
			htmltext = getHtm(player, isPractice ? "30846-23.html" : "30846-17.html");
			htmltext = htmltext.replace("%table%", table);
		}
		
		htmltext = htmltext.replace("%player_name%", player.getName());
		htmltext = htmltext.replace("%player_card%", playerCard.toString());
		htmltext = npcCard == null ? htmltext.replace("%npc_card%", "No such card") : htmltext.replace("%npc_card%", npcCard.toString());
		if (isPlayerWin)
		{
			reset(false);
		}
		
		if (isNpcWin)
		{
			reset(true);
		}
		
		return htmltext;
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final QuestState qs = getQuestState(killer, false);
		if ((qs != null) && qs.isCond(1))
		{
			switch (getRandom(0, 5))
			{
				case 0:
				case 1:
				{
					break;
				}
				case 5:
				{
					giveItems(killer, SPIRIT_BEAD, 2);
					break;
				}
				default:
				{
					giveItems(killer, SPIRIT_BEAD, 1);
				}
			}
		}
	}
	
	private void reset(boolean resetCounter)
	{
		playerCard = null;
		npcCard = null;
		if (resetCounter)
		{
			winCount = 0;
		}
	}
	
	private Card getRandomCard()
	{
		final int index = getRandom(0, 7);
		return Arrays.stream(cards).filter(card -> (!card.equals(playerCard)) && (!card.equals(npcCard))).collect(Collectors.toList()).get(index);
	}
	
	private boolean isWinner(Card playerCard, Card npcCard, boolean playerIsLastDraw)
	{
		if ((npcCard == null) && playerIsLastDraw && (playerCard.getNumber() == 5))
		{
			return true;
		}
		
		if ((npcCard != null) && (playerCard.getSide().ordinal() == npcCard.getSide().ordinal()))
		{
			return (playerCard.getNumber() + npcCard.getNumber()) == 5;
		}
		else if (playerIsLastDraw)
		{
			return playerCard.getNumber() == 5;
		}
		else if (npcCard != null)
		{
			return npcCard.getNumber() == 5;
		}
		
		return false;
	}
	
	private void giveRewards(Player player, int consecutiveWin)
	{
		switch (consecutiveWin)
		{
			case 1:
			{
				rewardItems(player, 57, 40000);
				break;
			}
			case 2:
			{
				rewardItems(player, 57, 80000);
				break;
			}
			case 3:
			{
				rewardItems(player, 57, 110000);
				rewardItems(player, 955, 1); // Enchant Weapon D
				break;
			}
			case 4:
			{
				rewardItems(player, 57, 199000);
				rewardItems(player, 951, 1); // Enchant Weapon C
				break;
			}
			case 5:
			{
				rewardItems(player, 57, 388000);
				rewardItems(player, getRandomRecipeBGrade(), 1); // Recipe Weapon B grade 60%
				break;
			}
			case 6:
			{
				rewardItems(player, 57, 675000);
				rewardItems(player, getRandomPartBGrade(), 1); // one part for making weapon b grade
				break;
			}
			case 7:
			{
				rewardItems(player, 57, 1284000);
				rewardItems(player, 947, 2); // Enchant Weapon B
				rewardItems(player, 948, 2); // Enchant Armor B
				break;
			}
			case 8:
			{
				rewardItems(player, 57, 2384000);
				rewardItems(player, 729, 1); // Enchant Weapon A
				rewardItems(player, 730, 2); // Enchant Armor A
				break;
			}
		}
	}
	
	private int getRandomRecipeBGrade()
	{
		return RECIPES_B_GRADE[getRandom(0, RECIPES_B_GRADE.length - 1)];
	}
	
	private int getRandomPartBGrade()
	{
		return PART_B_GRADE[getRandom(0, PART_B_GRADE.length - 1)];
	}
	
	private enum Side
	{
		SUN("Sun card"),
		MOON("Moon card");
		
		private String _name;
		
		Side(String name)
		{
			_name = name;
		}
		
		public String getName()
		{
			return _name;
		}
	}
	
	private static class Card
	{
		private final int _number;
		private final Side _side;
		
		public Card(Side side, int number)
		{
			_side = side;
			_number = number;
		}
		
		public Side getSide()
		{
			return _side;
		}
		
		public int getNumber()
		{
			return _number;
		}
		
		@Override
		public boolean equals(Object o)
		{
			return (o instanceof Card) && (((Card) o).getNumber() == _number) && (((Card) o).getSide().ordinal() == _side.ordinal());
		}
		
		@Override
		public String toString()
		{
			return _side == Side.MOON ? "<font color=\"LEVEL\">" + _side.getName() + ": " + _number + "</font>" : "<font color=\"FF0000\">" + _side.getName() + ": " + _number + "</font>";
		}
	}
}
