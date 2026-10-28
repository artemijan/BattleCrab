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
package ai.others.ClanHallAuctioneer;

import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.time.format.DateTimeFormatterBuilder;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.TimeUnit;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Collectors;

import org.l2jmobius.gameserver.data.xml.ClanHallData;
import org.l2jmobius.gameserver.managers.ClanHallAuctionManager;
import org.l2jmobius.gameserver.model.actor.Npc;
import org.l2jmobius.gameserver.model.actor.Player;
import org.l2jmobius.gameserver.model.clan.Clan;
import org.l2jmobius.gameserver.model.clan.ClanAccess;
import org.l2jmobius.gameserver.model.html.PageBuilder;
import org.l2jmobius.gameserver.model.html.PageResult;
import org.l2jmobius.gameserver.model.html.formatters.BypassParserFormatter;
import org.l2jmobius.gameserver.model.html.pagehandlers.NextPrevPageHandler;
import org.l2jmobius.gameserver.model.html.styles.ButtonsStyle;
import org.l2jmobius.gameserver.model.item.enums.ItemProcessType;
import org.l2jmobius.gameserver.model.itemcontainer.Inventory;
import org.l2jmobius.gameserver.model.residences.Bidder;
import org.l2jmobius.gameserver.model.residences.ClanHall;
import org.l2jmobius.gameserver.model.residences.ClanHallAuction;
import org.l2jmobius.gameserver.model.script.Script;
import org.l2jmobius.gameserver.network.SystemMessageId;
import org.l2jmobius.gameserver.network.serverpackets.NpcHtmlMessage;

/**
 * Clan Hall Auctioneer AI.
 * @author Sdw, Mobius
 */
public class ClanHallAuctioneer extends Script
{
	// NPC
	private static final int AUCTIONEER = 30767; // Auctioneer
	
	public ClanHallAuctioneer()
	{
		addStartNpc(AUCTIONEER);
		addTalkId(AUCTIONEER);
		addFirstTalkId(AUCTIONEER);
	}
	
	@Override
	public String onEvent(String event, Npc npc, Player player)
	{
		String htmltext = null;
		switch (event)
		{
			case "ClanHallAuctioneer.html":
			{
				htmltext = event;
				break;
			}
			case "map":
			{
				final String fnAgitMap = npc.getParameters().getString("fnAgitMap", "gludio");
				htmltext = getHtm(player, "ClanHallAuctioneer-map.html");
				htmltext = htmltext.replace("%MAP%", fnAgitMap);
				htmltext = htmltext.replace("%TOWN_NAME%", getTownName(fnAgitMap));
				break;
			}
			case "cancelBid":
			{
				final Clan clan = player.getClan();
				if (clan == null)
				{
					player.sendPacket(SystemMessageId.YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_PARTICIPATE_IN_AN_AUCTION);
					return htmltext;
				}
				
				if (!player.isClanLeader() || (clan.getLevel() < 4))
				{
					player.sendPacket(SystemMessageId.YOU_CAN_PARTICIPATE_IN_THE_CLAN_HALL_AUCTION_IF_YOUR_CLAN_LEVEL_IS_4_OR_ABOVE_AND_YOU_ARE_THE_CLAN_LEADER_OR_HAVE_THE_AUCTION_RIGHT);
					return htmltext;
				}
				
				final ClanHallAuction clanHallAuction = ClanHallAuctionManager.getInstance().getClanHallAuctionByClan(clan);
				if (clanHallAuction == null)
				{
					player.sendPacket(SystemMessageId.THERE_ARE_NO_OFFERINGS_I_OWN_OR_I_MADE_A_BID_FOR);
					return htmltext;
				}
				
				// THE_CLAN_DOES_NOT_OWN_A_CLAN_HALL
				
				htmltext = getHtm(player, "ClanHallAuctioneer-cancelBid.html");
				htmltext = htmltext.replace("%myBid%", String.valueOf(clanHallAuction.getClanBid(clan)));
				htmltext = htmltext.replace("%myBidRemain%", String.valueOf(clanHallAuction.getClanBid(clan) * 9));
				break;
			}
			case "cancel":
			{
				final Clan clan = player.getClan();
				if (clan == null)
				{
					player.sendPacket(SystemMessageId.YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_PARTICIPATE_IN_AN_AUCTION);
					return htmltext;
				}
				
				if (!player.isClanLeader() || (clan.getLevel() < 4))
				{
					player.sendPacket(SystemMessageId.YOU_CAN_PARTICIPATE_IN_THE_CLAN_HALL_AUCTION_IF_YOUR_CLAN_LEVEL_IS_4_OR_ABOVE_AND_YOU_ARE_THE_CLAN_LEADER_OR_HAVE_THE_AUCTION_RIGHT);
					return htmltext;
				}
				
				final ClanHallAuction clanHallAuction = ClanHallAuctionManager.getInstance().getClanHallAuctionByClan(clan);
				if (clanHallAuction == null)
				{
					player.sendPacket(SystemMessageId.THERE_ARE_NO_OFFERINGS_I_OWN_OR_I_MADE_A_BID_FOR);
					return htmltext;
				}
				
				// THE_CLAN_DOES_NOT_OWN_A_CLAN_HALL
				
				clanHallAuction.removeBid(clan);
				
				player.sendPacket(SystemMessageId.YOU_HAVE_CANCELED_YOUR_BID);
				break;
			}
			case "rebid":
			{
				if (player.hasAccess(ClanAccess.HALL_AUCTION))
				{
					final Clan clan = player.getClan();
					final ClanHallAuction clanHallAuction = ClanHallAuctionManager.getInstance().getClanHallAuctionByClan(clan);
					if (clanHallAuction != null)
					{
						final DateTimeFormatterBuilder builder = new DateTimeFormatterBuilder();
						
						htmltext = getHtm(player, "ClanHallAuctioneer-bid2.html");
						htmltext = htmltext.replace("%id%", String.valueOf(clanHallAuction.getClanHallId()));
						htmltext = htmltext.replace("%minBid%", String.valueOf(clanHallAuction.getHighestBid()));
						htmltext = htmltext.replace("%myBid%", String.valueOf(clanHallAuction.getClanBid(clan)));
						htmltext = htmltext.replace("%auctionEnd%", builder.appendPattern("dd/MM/yyyy HH").appendLiteral(" hour ").appendPattern("mm").appendLiteral(" minutes").toFormatter().format(Instant.ofEpochMilli(System.currentTimeMillis() + ClanHallAuctionManager.getInstance().getRemainingTime()).atZone(ZoneId.systemDefault())));
					}
				}
				else
				{
					player.sendPacket(SystemMessageId.YOU_MUST_HAVE_RIGHTS_TO_A_CLAN_HALL_AUCTION_IN_ORDER_TO_MAKE_A_BID_FOR_PROVISIONAL_CLAN_HALL); // FIX ME
				}
				break;
			}
			case "my_auction":
			{
				final Clan clan = player.getClan();
				if (clan == null)
				{
					player.sendPacket(SystemMessageId.YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_PARTICIPATE_IN_AN_AUCTION);
					return htmltext;
				}
				
				if (!player.isClanLeader() || (clan.getLevel() < 4))
				{
					player.sendPacket(SystemMessageId.YOU_CAN_PARTICIPATE_IN_THE_CLAN_HALL_AUCTION_IF_YOUR_CLAN_LEVEL_IS_4_OR_ABOVE_AND_YOU_ARE_THE_CLAN_LEADER_OR_HAVE_THE_AUCTION_RIGHT);
					return htmltext;
				}
				
				final ClanHallAuction clanHallAuction = ClanHallAuctionManager.getInstance().getClanHallAuctionByClan(clan);
				if (clanHallAuction == null)
				{
					player.sendPacket(SystemMessageId.THERE_ARE_NO_OFFERINGS_I_OWN_OR_I_MADE_A_BID_FOR);
					return htmltext;
				}
				
				// THE_CLAN_DOES_NOT_OWN_A_CLAN_HALL
				
				final ClanHall clanHall = ClanHallData.getInstance().getClanHallById(clanHallAuction.getClanHallId());
				final Clan owner = clanHall.getOwner();
				final long remainingTime = ClanHallAuctionManager.getInstance().getRemainingTime();
				final Instant endTime = Instant.ofEpochMilli(System.currentTimeMillis() + remainingTime);
				
				final DateTimeFormatterBuilder builder = new DateTimeFormatterBuilder();
				htmltext = getHtm(player, "ClanHallAuctioneer-bidInfo.html");
				htmltext = htmltext.replace("%id%", String.valueOf(clanHall.getResidenceId()));
				htmltext = htmltext.replace("%owner%", owner != null ? owner.getName() : "");
				htmltext = htmltext.replace("%clanLeader%", owner != null ? owner.getLeaderName() : "");
				htmltext = htmltext.replace("%rent%", String.valueOf(clanHall.getLease()));
				htmltext = htmltext.replace("%grade%", String.valueOf(clanHall.getGrade().getGradeValue()));
				htmltext = htmltext.replace("%minBid%", String.valueOf(clanHallAuction.getHighestBid()));
				htmltext = htmltext.replace("%myBid%", String.valueOf(clanHallAuction.getClanBid(clan)));
				htmltext = htmltext.replace("%bidNumber%", String.valueOf(clanHallAuction.getBidCount()));
				htmltext = htmltext.replace("%auctionEnd%", builder.appendPattern("dd/MM/yyyy HH").appendLiteral(" hour ").appendPattern("mm").appendLiteral(" minutes").toFormatter().format(endTime.atZone(ZoneId.systemDefault())));
				htmltext = htmltext.replace("%hours%", String.valueOf(TimeUnit.MILLISECONDS.toHours(remainingTime)));
				htmltext = htmltext.replace("%minutes%", String.valueOf(TimeUnit.MILLISECONDS.toMinutes(remainingTime % 3600000)));
				break;
			}
			default:
			{
				if (event.startsWith("auctionList"))
				{
					processClanHallBypass(player, npc, event);
				}
				else if (event.startsWith("bid"))
				{
					processBidBypass(player, npc, event);
				}
				else if (event.startsWith("listBidder"))
				{
					processBiddersBypass(player, npc, event);
				}
				break;
			}
		}
		
		return htmltext;
	}
	
	@Override
	public String onFirstTalk(Npc npc, Player player)
	{
		return "ClanHallAuctioneer.html";
	}
	
	private void processClanHallBypass(Player player, Npc npc, String event)
	{
		final int page = parseInt(event, "page", 0);
		final int clanHallId = parseInt(event, "id", 0);
		
		if (clanHallId > 0)
		{
			final ClanHall clanHall = ClanHallData.getInstance().getClanHallById(clanHallId);
			if (clanHall != null)
			{
				final ClanHallAuction clanHallAuction = ClanHallAuctionManager.getInstance().getClanHallAuctionById(clanHallId);
				final Clan owner = clanHall.getOwner();
				final long remainingTime = ClanHallAuctionManager.getInstance().getRemainingTime();
				final Instant endTime = Instant.ofEpochMilli(System.currentTimeMillis() + remainingTime);
				final DateTimeFormatterBuilder builder = new DateTimeFormatterBuilder();
				final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId());
				html.setHtml(getHtm(player, "ClanHallAuctioneer-info.html"));
				
				html.replace("%id%", clanHall.getResidenceId());
				html.replace("%owner%", owner != null ? owner.getName() : "");
				html.replace("%clanLeader%", owner != null ? owner.getLeaderName() : "");
				html.replace("%rent%", clanHall.getLease());
				html.replace("%grade%", clanHall.getGrade().getGradeValue());
				html.replace("%minBid%", clanHallAuction.getHighestBid());
				html.replace("%bidNumber%", clanHallAuction.getBidCount());
				html.replace("%auctionEnd%", builder.appendPattern("dd/MM/yyyy HH").appendLiteral(" hour ").appendPattern("mm").appendLiteral(" minutes").toFormatter().format(endTime.atZone(ZoneId.systemDefault())));
				html.replace("%hours%", TimeUnit.MILLISECONDS.toHours(remainingTime));
				html.replace("%minutes%", TimeUnit.MILLISECONDS.toMinutes(remainingTime % 3600000));
				player.sendPacket(html);
			}
		}
		else
		{
			final List<ClanHall> clanHalls = ClanHallData.getInstance().getFreeAuctionableHall();
			if (clanHalls.isEmpty())
			{
				player.sendPacket(SystemMessageId.THERE_ARE_NO_CLAN_HALLS_UP_FOR_AUCTION);
			}
			else
			{
				final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId(), getHtm(player, "ClanHallAuctioneer-list.html"));
				// @formatter:off
				final PageResult result = PageBuilder.newBuilder(clanHalls, 8, "bypass -h Script ClanHallAuctioneer auctionList")
					.currentPage(page)
					.pageHandler(NextPrevPageHandler.INSTANCE)
					.formatter(BypassParserFormatter.INSTANCE)
					.style(ButtonsStyle.INSTANCE)
					.bodyHandler((pages, clanHall, sb) ->
				{
					final ClanHallAuction auction = ClanHallAuctionManager.getInstance().getClanHallAuctionById(clanHall.getResidenceId());
					if(auction == null)
					{
						// System.out.println(clanHall.getResidenceId());
						return;
					}
					
					sb.append("<tr><td width=50><font color=\"aaaaff\">&^");
					sb.append(clanHall.getResidenceId());
					sb.append(";</font></td><td width=100><a action=\"bypass -h Script ClanHallAuctioneer auctionList id=");
					sb.append(clanHall.getResidenceId());
					sb.append("\"><font color=\"ffffaa\">&%");
					sb.append(clanHall.getResidenceId());
					sb.append(";[0]</font></a></td><td width=50>");
					sb.append(DateTimeFormatter.ofPattern("dd/MM/yyyy").format(Instant.ofEpochMilli(System.currentTimeMillis() + ClanHallAuctionManager.getInstance().getRemainingTime()).atZone(ZoneId.systemDefault())));
					sb.append("</td><td width=70 align=right><font color=\"aaffff\">");
					sb.append(auction.getHighestBid());
					sb.append("</font></td></tr>");
				}).build();
				// @formatter:on
				html.replace("%pages%", result.getPages() > 0 ? result.getPagerTemplate() : "");
				html.replace("%agitList%", result.getBodyTemplate().toString());
				player.sendPacket(html);
			}
		}
	}
	
	private void processBidBypass(Player player, Npc npc, String event)
	{
		final int clanHallId = parseInt(event, "id", 0);
		final long bid = parseLong(event, "bid", 0);
		
		if (clanHallId > 0)
		{
			final ClanHall clanHall = ClanHallData.getInstance().getClanHallById(clanHallId);
			if (clanHall == null)
			{
				return;
			}
			
			final Clan clan = player.getClan();
			if (clan == null)
			{
				player.sendPacket(SystemMessageId.YOU_DO_NOT_MEET_THE_REQUIREMENTS_TO_PARTICIPATE_IN_AN_AUCTION);
				return;
			}
			
			if (!player.isClanLeader() || (clan.getLevel() < 4))
			{
				player.sendPacket(SystemMessageId.YOU_CAN_PARTICIPATE_IN_THE_CLAN_HALL_AUCTION_IF_YOUR_CLAN_LEVEL_IS_4_OR_ABOVE_AND_YOU_ARE_THE_CLAN_LEADER_OR_HAVE_THE_AUCTION_RIGHT);
				return;
			}
			
			final ClanHall playerClanHall = ClanHallData.getInstance().getClanHallByClan(clan);
			if (playerClanHall != null)
			{
				player.sendPacket(SystemMessageId.YOU_ALREADY_HAVE_A_CLAN_HALL_SO_YOU_CANNOT_BID);
				return;
			}
			
			if (ClanHallAuctionManager.getInstance().checkForClanBid(clanHallId, clan))
			{
				player.sendPacket(SystemMessageId.SINCE_YOU_HAVE_ALREADY_SUBMITTED_A_BID_YOU_ARE_NOT_ALLOWED_TO_PARTICIPATE_IN_ANOTHER_AUCTION_AT_THIS_TIME);
				return;
			}
			
			if (bid == 0)
			{
				final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId());
				html.setHtml(getHtm(player, "ClanHallAuctioneer-bid1.html"));
				html.replace("%clanAdena%", clan.getWarehouse().getAdena());
				html.replace("%minBid%", ClanHallAuctionManager.getInstance().getClanHallAuctionById(clanHallId).getHighestBid());
				html.replace("%id%", clanHall.getResidenceId());
				player.sendPacket(html);
			}
			else
			{
				player.sendPacket(SystemMessageId.YOU_HAVE_REGISTERED_FOR_A_CLAN_HALL_AUCTION);
				if (bid > Inventory.MAX_ADENA)
				{
					player.sendPacket(SystemMessageId.THE_HIGHEST_BID_IS_OVER_999_9_BILLION_THEREFORE_YOU_CANNOT_PLACE_A_BID);
					return;
				}
				
				final ClanHallAuction auction = ClanHallAuctionManager.getInstance().getClanHallAuctionById(clanHallId);
				if (bid < auction.getHighestBid())
				{
					player.sendPacket(SystemMessageId.YOUR_BID_PRICE_MUST_BE_HIGHER_THAN_THE_MINIMUM_PRICE_CURRENTLY_BEING_BID);
					return;
				}
				else if (clan.getWarehouse().destroyItemByItemId(ItemProcessType.FEE, Inventory.ADENA_ID, bid, player, null) == null)
				{
					player.sendPacket(SystemMessageId.THERE_IS_NOT_ENOUGH_ADENA_IN_THE_CLAN_HALL_WAREHOUSE);
					return;
				}
				
				final Optional<Bidder> bidder = auction.getHighestBidder();
				if (bidder.isPresent())
				{
					auction.returnAdenas(bidder.get());
					final Player leader = bidder.get().getClan().getLeader().getPlayer();
					if ((leader != null) && leader.isOnline())
					{
						leader.sendPacket(SystemMessageId.YOU_WERE_OUTBID_THE_NEW_HIGHEST_BID_IS_S1_ADENA);
					}
				}
				
				auction.addBid(player.getClan(), bid);
				
				player.sendPacket(SystemMessageId.YOUR_BID_HAS_BEEN_SUCCESSFULLY_PLACED);
				
			}
		}
	}
	
	private void processBiddersBypass(Player player, Npc npc, String event)
	{
		final int page = parseInt(event, "page", 0);
		final int clanHallId = parseInt(event, "id", 0);
		
		if (clanHallId > 0)
		{
			final ClanHallAuction clanHallAuction = ClanHallAuctionManager.getInstance().getClanHallAuctionById(clanHallId);
			if (clanHallAuction == null)
			{
				return;
			}
			
			final NpcHtmlMessage html = new NpcHtmlMessage(npc.getObjectId(), getHtm(player, "ClanHallAuctioneer-bidderList.html"));
			// @formatter:off
			final PageResult result = PageBuilder.newBuilder(clanHallAuction.getBids().values().stream().sorted(Comparator.comparingLong(Bidder::getTime).reversed()).collect(Collectors.toList()), 10, "bypass -h Script ClanHallAuctioneer auctionList")
				.currentPage(page)
				.pageHandler(NextPrevPageHandler.INSTANCE)
				.formatter(BypassParserFormatter.INSTANCE)
				.style(ButtonsStyle.INSTANCE)
				.bodyHandler((pages, bidder, sb) ->
			{
				sb.append("<tr><td width=100>");
				sb.append(bidder.getClanName());
				sb.append("</td><td width=100>");
				sb.append(bidder.getBid());
				sb.append("</td><td width=70>");
				sb.append(bidder.getFormattedTime());
				sb.append("</td></tr>");
			}).build();
			// @formatter:on
			html.replace("%pages%", result.getPages() > 0 ? result.getPagerTemplate() : "");
			html.replace("%bidderList%", result.getBodyTemplate().toString());
			html.replace("%id%", clanHallAuction.getClanHallId());
			player.sendPacket(html);
		}
	}
	
	private int parseInt(String event, String key, int defaultValue)
	{
		final Pattern pattern = Pattern.compile(key + "=\\s*'?([^'\\s]+)'?");
		final Matcher matcher = pattern.matcher(event);
		if (matcher.find())
		{
			try
			{
				return Integer.parseInt(matcher.group(1).trim());
			}
			catch (NumberFormatException e)
			{
				// Ignore and return default.
			}
		}
		
		return defaultValue;
	}
	
	private long parseLong(String event, String key, long defaultValue)
	{
		final Pattern pattern = Pattern.compile(key + "='([^']+)'");
		final Matcher matcher = pattern.matcher(event);
		if (matcher.find())
		{
			try
			{
				return Long.parseLong(matcher.group(1).trim());
			}
			catch (NumberFormatException e)
			{
				// Ignore and return default.
			}
		}
		
		return defaultValue;
	}
	
	private String getTownName(String fnAgitMap)
	{
		String name = "";
		switch (fnAgitMap)
		{
			case "aden":
			{
				name = "Aden";
				break;
			}
			case "floran":
			{
				name = "Floran";
				break;
			}
			case "dion":
			case "dion01":
			{
				name = "Dion";
				break;
			}
			case "giran":
			{
				name = "Giran";
				break;
			}
			case "gludin":
			{
				name = "Gludin";
				break;
			}
			case "gludio":
			case "gludio01":
			{
				name = "Gludio";
				break;
			}
			case "godard":
			{
				name = "Goddard";
				break;
			}
			case "rune":
			{
				name = "Rune";
				break;
			}
			case "schuttgart":
			{
				name = "Schuttgart";
				break;
			}
		}
		
		return name;
	}
	
	public static void main(String[] args)
	{
		new ClanHallAuctioneer();
	}
}
