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
package quests.Q00619_RelicsOfTheOldEmpire;

import org.l2jmobius.gameserver.enums.QuestSound;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;
import org.l2jmobius.gameserver.model.quest.State;

public class Q00619_RelicsOfTheOldEmpire extends Quest {
  // NPC
  private static final int GHOST_OF_ADVENTURER = 31538;
  // Items
  private static final int RELICS = 7254;
  private static final int ENTRANCE = 7075;
  // Rewards ; all S grade weapons recipe (60%)
  private static final int[] RCP_REWARDS = new int[]
    {
      6881,
      6883,
      6885,
      6887,
      6891,
      6893,
      6895,
      6897,
      6899,
      7580
    };

  public Q00619_RelicsOfTheOldEmpire() {
    super(619);
    registerQuestItems(RELICS);
    addStartNpc(GHOST_OF_ADVENTURER);
    addTalkId(GHOST_OF_ADVENTURER);
    for (int id = 21396; id <= 21434; id++) {
      // IT monsters
      addKillId(id);
    }
    // monsters at IT entrance
    addKillId(21798, 21799, 21800);
    for (int id = 18120; id <= 18256; id++) {
      // Sepulchers monsters
      addKillId(id);
    }
  }

  @Override
  public String onEvent(String event, Npc npc, Player player) {
    String htmltext = event;
    final QuestState st = getQuestState(player, false);
    if (st == null) {
      return htmltext;
    }

    switch (event) {
      case "31538-03.htm": {
        st.startQuest();
        break;
      }
      case "31538-09.htm": {
        if (getQuestItemsCount(player, RELICS) >= 1000) {
          htmltext = "31538-09.htm";
          takeItems(player, RELICS, 1000);
          giveItems(player, RCP_REWARDS[getRandom(RCP_REWARDS.length)], 1);
        } else {
          htmltext = "31538-06.htm";
        }
        break;
      }
      case "31538-10.htm": {
        st.exitQuest(true, true);
        break;
      }
    }
    return htmltext;
  }

  @Override
  public String onTalk(Npc npc, Player player) {
    String htmltext = getNoQuestMsg(player);
    final QuestState st = getQuestState(player, true);

    switch (st.getState()) {
      case State.CREATED: {
        htmltext = (player.getLevel() < 74) ? "31538-02.htm" : "31538-01.htm";
        break;
      }
      case State.STARTED: {
        if (getQuestItemsCount(player, RELICS) >= 1000) {
          htmltext = "31538-04.htm";
        } else if (hasQuestItems(player, ENTRANCE)) {
          htmltext = "31538-06.htm";
        } else {
          htmltext = "31538-07.htm";
        }
        break;
      }
    }

    return htmltext;
  }

  @Override
  public String onKill(Npc npc, Player player, boolean isPet) {
    final QuestState st = getRandomPartyMemberState(player, -1, 3, npc);
    if ((st == null) || !st.isStarted()) {
      return null;
    }
    final Player partyMember = st.getPlayer();
    if (getRandomBoolean()) {
      giveItems(partyMember, RELICS, 2);
    } else {
      giveItems(partyMember, RELICS, 1);
    }
    playSound(partyMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
    if (getRandom(100) <= 10) {
      giveItems(partyMember, ENTRANCE, 1);
    }
    return null;
  }
}
