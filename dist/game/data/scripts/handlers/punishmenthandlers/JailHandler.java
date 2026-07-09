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
package handlers.punishmenthandlers;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.LoginServerThread;
import org.l2jmobius.gameserver.cache.HtmCache;
import org.l2jmobius.gameserver.handler.IPunishmentHandler;
import org.l2jmobius.gameserver.model.World;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.actor.tasks.player.TeleportTask;
import org.l2jmobius.gameserver.model.events.Containers;
import org.l2jmobius.gameserver.model.events.EventType;
import org.l2jmobius.gameserver.model.events.impl.creature.player.OnPlayerLogin;
import org.l2jmobius.gameserver.model.events.listeners.ConsumerEventListener;
import org.l2jmobius.gameserver.model.olympiad.OlympiadManager;
import org.l2jmobius.gameserver.model.punishment.PunishmentTask;
import org.l2jmobius.gameserver.model.punishment.PunishmentType;
import org.l2jmobius.gameserver.model.zone.ZoneId;
import org.l2jmobius.gameserver.model.zone.type.JailZone;
import org.l2jmobius.gameserver.network.GameClient;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

/**
 * This class handles jail punishment.
 * @author UnAfraid
 */
public class JailHandler implements IPunishmentHandler
{
	public JailHandler()
	{
		// Register global listener
		Containers.Global().addListener(new ConsumerEventListener(Containers.Global(), EventType.ON_PLAYER_LOGIN, (OnPlayerLogin event) -> onPlayerLogin(event), this));
	}
	
	private void onPlayerLogin(OnPlayerLogin event)
	{
		final Player player = event.getPlayer();
		if (player.isJailed() && !player.isInsideZone(ZoneId.JAIL))
		{
			applyToPlayer(null, player);
		}
		else if (!player.isJailed() && player.isInsideZone(ZoneId.JAIL) && !player.isGM())
		{
			removeFromPlayer(player);
		}
	}
	
	@Override
	public void onStart(PunishmentTask task)
	{
		switch (task.getAffect())
		{
			case CHARACTER:
			{
				final int objectId = Integer.parseInt(String.valueOf(task.getKey()));
				final Player player = World.getInstance().getPlayer(objectId);
				if (player != null)
				{
					applyToPlayer(task, player);
				}
				break;
			}
			case ACCOUNT:
			{
				final String account = String.valueOf(task.getKey());
				final GameClient client = LoginServerThread.getInstance().getClient(account);
				if (client != null)
				{
					final Player player = client.getPlayer();
					if (player != null)
					{
						applyToPlayer(task, player);
					}
				}
				break;
			}
			case IP:
			{
				final String ip = String.valueOf(task.getKey());
				for (Player player : World.getInstance().getPlayers())
				{
					if (player.getIPAddress().equals(ip))
					{
						applyToPlayer(task, player);
					}
				}
				break;
			}
			case HWID:
			{
				final String hwid = String.valueOf(task.getKey());
				for (Player player : World.getInstance().getPlayers())
				{
					final GameClient client = player.getClient();
					if ((client != null) && client.getHardwareInfo().getMacAddress().equals(hwid))
					{
						applyToPlayer(task, player);
					}
				}
				break;
			}
		}
	}
	
	@Override
	public void onEnd(PunishmentTask task)
	{
		switch (task.getAffect())
		{
			case CHARACTER:
			{
				final int objectId = Integer.parseInt(String.valueOf(task.getKey()));
				final Player player = World.getInstance().getPlayer(objectId);
				if (player != null)
				{
					removeFromPlayer(player);
				}
				break;
			}
			case ACCOUNT:
			{
				final String account = String.valueOf(task.getKey());
				final GameClient client = LoginServerThread.getInstance().getClient(account);
				if (client != null)
				{
					final Player player = client.getPlayer();
					if (player != null)
					{
						removeFromPlayer(player);
					}
				}
				break;
			}
			case IP:
			{
				final String ip = String.valueOf(task.getKey());
				for (Player player : World.getInstance().getPlayers())
				{
					if (player.getIPAddress().equals(ip))
					{
						removeFromPlayer(player);
					}
				}
				break;
			}
			case HWID:
			{
				final String hwid = String.valueOf(task.getKey());
				for (Player player : World.getInstance().getPlayers())
				{
					final GameClient client = player.getClient();
					if ((client != null) && client.getHardwareInfo().getMacAddress().equals(hwid))
					{
						removeFromPlayer(player);
					}
				}
				break;
			}
		}
	}
	
	/**
	 * Applies all punishment effects from the player.
	 * @param task
	 * @param player
	 */
	private void applyToPlayer(PunishmentTask task, Player player)
	{
		player.setInstance(null);
		
		if (OlympiadManager.getInstance().isRegisteredInComp(player))
		{
			OlympiadManager.getInstance().removeDisconnectedCompetitor(player);
		}
		
		ThreadPool.schedule(new TeleportTask(player, JailZone.getLocationIn()), 2000);
		
		// Open a Html message to inform the player
		final NpcHtmlMessage msg = new NpcHtmlMessage();
		String content = HtmCache.getInstance().getHtm(player, "data/html/jail_in.htm");
		if (content != null)
		{
			content = content.replace("%reason%", task != null ? task.getReason() : "");
			content = content.replace("%punishedBy%", task != null ? task.getPunishedBy() : "");
			msg.setHtml(content);
		}
		else
		{
			msg.setHtml("<html><body>You have been put in jail by an admin.</body></html>");
		}
		player.sendPacket(msg);
		if (task != null)
		{
			final long delay = (task.getExpirationTime() - System.currentTimeMillis()) / 1000;
			if (delay > 0)
			{
				player.sendMessage("You've been jailed for " + (delay > 60 ? ((delay / 60) + " minutes.") : delay + " seconds."));
			}
			else
			{
				player.sendMessage("You've been jailed forever.");
			}
		}
	}
	
	/**
	 * Removes any punishment effects from the player.
	 * @param player
	 */
	private void removeFromPlayer(Player player)
	{
		ThreadPool.schedule(new TeleportTask(player, JailZone.getLocationOut()), 2000);
		
		// Open a Html message to inform the player
		final NpcHtmlMessage msg = new NpcHtmlMessage();
		final String content = HtmCache.getInstance().getHtm(player, "data/html/jail_out.htm");
		if (content != null)
		{
			msg.setHtml(content);
		}
		else
		{
			msg.setHtml("<html><body>You are free for now, respect server rules!</body></html>");
		}
		player.sendPacket(msg);
	}
	
	@Override
	public PunishmentType getType()
	{
		return PunishmentType.JAIL;
	}
}
