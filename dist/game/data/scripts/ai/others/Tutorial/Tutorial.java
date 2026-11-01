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
package ai.others.Tutorial;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.config.PlayerConfig;
import org.l2jmobius.gameserver.data.enums.CategoryType;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.enums.creature.Race;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.ListenerRegisterType;
import org.l2jmobius.gameserver.model.events.annotations.RegisterEvent;
import org.l2jmobius.gameserver.model.events.annotations.RegisterType;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerBypass;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerCreate;
import org.l2jmobius.gameserver.model.events.holders.actor.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.serverpackets.ExTutorialShowId;
import org.l2jmobius.gameserver.network.serverpackets.PlaySound;
import org.l2jmobius.gameserver.network.serverpackets.TutorialCloseHtml;
import org.l2jmobius.gameserver.network.serverpackets.TutorialShowHtml;

/**
 * Tutorial
 * @author Gigi
 * @date 2019-08-21 - [21:06:44]
 */
public class Tutorial extends Script
{
	// Misc
	private static final String TUTORIAL_VAR = "TUTORIAL";
	
	private Tutorial()
	{
	}
	
	@RegisterEvent(EventType.ON_PLAYER_LOGIN)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		if (player.isInCategory(CategoryType.ALLOWED_BALTHUS_CLASSES))
		{
			return;
		}
		
		if (player.getVariables().getInt(TUTORIAL_VAR, 0) == 0)
		{
			if (player.getRace() == Race.ERTHEIA)
			{
				ThreadPool.schedule(() ->
				{
					final String html = getHtm(player, "tutorial_01_ertheia.html");
					player.sendPacket(new TutorialShowHtml(html));
					player.sendPacket(new ExTutorialShowId(5));
				}, 26000);
			}
			else
			{
				ThreadPool.schedule(() ->
				{
					final String html = getHtm(player, "tutorial_01.html");
					player.sendPacket(new TutorialShowHtml(html));
					player.sendPacket(new ExTutorialShowId(5));
				}, 26000);
			}
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_BYPASS)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerBypass(OnPlayerBypass event)
	{
		final Player player = event.getPlayer();
		if (event.getCommand().equals("chat_window"))
		{
			ThreadPool.schedule(() ->
			{
				player.sendPacket(new ExTutorialShowId(1));
				player.sendPacket(new PlaySound(2, "tutorial_voice_006", 0, 0, 0, 0, 0));
			}, 500);
			player.getVariables().set(TUTORIAL_VAR, 1);
			player.sendPacket(TutorialCloseHtml.STATIC_PACKET);
		}
	}
	
	@RegisterEvent(EventType.ON_PLAYER_CREATE)
	@RegisterType(ListenerRegisterType.GLOBAL_PLAYERS)
	public void onPlayerCreate(OnPlayerCreate event)
	{
		final Player player = event.getPlayer();
		if (PlayerConfig.DISABLE_TUTORIAL)
		{
			return;
		}
		
		player.getVariables().set(TUTORIAL_VAR, 0);
	}
	
	public static void main(String[] args)
	{
		new Tutorial();
	}
}
