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
package ai.others.FameManager;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.SystemMessageId;

/**
 * Fame Manager AI.
 * @author St3eT
 */
public class FameManager extends Script
{
	// Npc
	private static final int[] FAME_MANAGER =
	{
		36479, // Rapidus
		36480, // Scipio
	};
	
	// Misc
	private static final int MIN_LEVEL = 40;
	private static final int DECREASE_COST = 5000;
	private static final int REPUTATION_COST = 1000;
	private static final int MIN_CLAN_LEVEL = 5;
	private static final int CLASS_LEVEL = 2;
	
	private FameManager()
	{
		addStartNpc(FAME_MANAGER);
		addTalkId(FAME_MANAGER);
		addFirstTalkId(FAME_MANAGER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "36479.html":
			case "36479-02.html":
			case "36479-07.html":
			case "36480.html":
			case "36480-02.html":
			case "36480-07.html":
			{
				htmltext = event;
				break;
			}
			case "decreasePk":
			{
				if (player.getPkKills() > 0)
				{
					if ((player.getFame() >= DECREASE_COST) && (player.getLevel() >= MIN_LEVEL) && (player.getPlayerClass().level() >= CLASS_LEVEL))
					{
						player.setFame(player.getFame() - DECREASE_COST);
						player.setPkKills(player.getPkKills() - 1);
						player.updateUserInfo();
						htmltext = npc.getId() + "-06.html";
					}
					else
					{
						htmltext = npc.getId() + "-01.html";
					}
				}
				else
				{
					htmltext = npc.getId() + "-05.html";
				}
				break;
			}
			case "clanRep":
			{
				if ((player.getClan() != null) && (player.getClan().getLevel() >= MIN_CLAN_LEVEL))
				{
					if ((player.getFame() >= REPUTATION_COST) && (player.getLevel() >= MIN_LEVEL) && (player.getPlayerClass().level() >= CLASS_LEVEL))
					{
						player.setFame(player.getFame() - REPUTATION_COST);
						player.getClan().addReputationScore(50);
						player.updateUserInfo();
						player.sendPacket(SystemMessageId.YOU_HAVE_ACQUIRED_50_CLAN_REPUTATION);
						htmltext = npc.getId() + "-04.html";
					}
					else
					{
						htmltext = npc.getId() + "-01.html";
					}
				}
				else
				{
					htmltext = npc.getId() + "-03.html";
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return ((player.getFame() > 0) && (player.getLevel() >= MIN_LEVEL) && (player.getPlayerClass().level() >= CLASS_LEVEL)) ? npc.getId() + ".html" : npc.getId() + "-01.html";
	}
	
	public static void main(String[] args)
	{
		new FameManager();
	}
}
