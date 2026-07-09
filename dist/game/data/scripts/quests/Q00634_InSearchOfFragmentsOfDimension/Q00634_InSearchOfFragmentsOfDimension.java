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
package quests.Q00634_InSearchOfFragmentsOfDimension;

import org.l2jmobius.gameserver.enums.QuestSound;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.quest.Quest;
import org.l2jmobius.gameserver.model.quest.QuestState;
import org.l2jmobius.gameserver.model.quest.State;

import java.util.ArrayList;

public class Q00634_InSearchOfFragmentsOfDimension extends Quest {
  // Items
  private static final int DIMENSION_FRAGMENT = 7079;

  public Q00634_InSearchOfFragmentsOfDimension() {
    super(634);

    // Dimensional Gate Keepers. 31147
    ArrayList<Integer> nonExistingIds = new ArrayList<>() {
      {
        add(31150);
        add(31147);
        add(31151);
        add(31152);
        add(31153);
        add(31154);
        add(31155);
        add(31156);
        add(31157);
        add(31158);
        add(31159);
        add(31160);
        add(31161);
        add(31162);
        add(31163);
        add(31164);
        add(31165);
        add(31166);
        add(31167);
      }
    };
    for (int i = 31095; i < 31195; i++) {
      if (!nonExistingIds.contains(i)) {
        addStartNpc(i);
        addTalkId(i);
      }
    }

    // Only aggressive mobs
    // 21139 - 21165 [A][G]
    for (int i = 21139; i <= 21165; i++) {
      addKillId(i);
    }
    // 21208 - 21255
    for (int i = 21208; i <= 21255; i++) {
      addKillId(i);
    }

  }

  @Override
  public String onEvent(String event, Npc npc, Player player) {
    final QuestState st = getQuestState(player, false);
    if (st == null) {
      return event;
    }

    if (event.equals("02.htm")) {
      st.startQuest();
    } else if (event.equals("05.htm")) {
      st.exitQuest(true, true);
    }

    return event;
  }

  @Override
  public String onTalk(Npc npc, Player player) {
    String htmltext = getNoQuestMsg(player);
    final QuestState st = getQuestState(player, true);

    switch (st.getState()) {
      case State.CREATED: {
        htmltext = (player.getLevel() < 20) ? "01a.htm" : "01.htm";
        break;
      }
      case State.STARTED: {
        htmltext = "03.htm";
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

    if (getRandom(100) < 80) {
      giveItems(partyMember, DIMENSION_FRAGMENT, (int) ((npc.getLevel() * 0.15) + 2.6));
      playSound(partyMember, QuestSound.ITEMSOUND_QUEST_ITEMGET);
    }

    return null;
  }
}
