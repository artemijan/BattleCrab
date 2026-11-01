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
package ai.areas.DenOfEvil;

import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.holders.npc.MinionHolder;
import org.l2jmobius.gameserver.model.script.Script;

/**
 * Ragna Orc Commander AI.
 * @author Zealar, Mobius
 */
public class RagnaOrcCommander extends Script
{
	private static final int RAGNA_ORC_COMMANDER = 22694;
	
	private RagnaOrcCommander()
	{
		addSpawnId(RAGNA_ORC_COMMANDER);
	}
	
	@Override
	public void onSpawn(Npc npc)
	{
		for (MinionHolder minionHolder : npc.getParameters().getMinionList("Privates1"))
		{
			addMinion(npc.asMonster(), minionHolder.getId());
		}
		
		for (MinionHolder minionHolder : npc.getParameters().getMinionList(getRandomBoolean() ? "Privates2" : "Privates3"))
		{
			addMinion(npc.asMonster(), minionHolder.getId());
		}
	}
	
	public static void main(String[] args)
	{
		new RagnaOrcCommander();
	}
}
