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
package ai.others;

import java.util.ArrayList;
import java.util.List;

import org.l2jmobius.gameserver.data.holders.MonsterBookCardHolder;
import org.l2jmobius.gameserver.data.xml.MonsterBookData;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.groups.Party;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * @author Mobius
 */
public class MonsterBook extends Script
{
	private static final int MAXIMUM_REWARD_RANGE = 2500;
	private static final int MINIMUM_PARTY_LEVEL = 99;
	
	private MonsterBook()
	{
		for (MonsterBookCardHolder card : MonsterBookData.getInstance().getMonsterBookCards())
		{
			addKillId(card.getMonsterId());
		}
	}
	
	@Override
	public void onKill(Npc npc, Player killer, boolean isSummon)
	{
		final Party party = killer.getParty();
		List<Player> rewardedPlayers = new ArrayList<>();
		if (party != null)
		{
			rewardedPlayers = party.isInCommandChannel() ? party.getCommandChannel().getMembers() : party.getMembers();
		}
		else
		{
			rewardedPlayers.add(killer);
		}
		
		final MonsterBookCardHolder card = MonsterBookData.getInstance().getMonsterBookCardByMonsterId(npc.getId());
		for (Player player : rewardedPlayers)
		{
			if (((player != null) && (player.calculateDistance2D(killer) < MAXIMUM_REWARD_RANGE)) && (player.getLevel() >= MINIMUM_PARTY_LEVEL))
			{
				player.updateMonsterBook(card);
			}
		}
	}
	
	public static void main(String[] args)
	{
		new MonsterBook();
	}
}
