DROP TABLE IF EXISTS `item_auction_bid`;
CREATE TABLE IF NOT EXISTS `item_auction_bid` (
  `auctionId` int NOT NULL,
  `playerObjId` int NOT NULL,
  `playerBid` bigint NOT NULL,
  PRIMARY KEY (`auctionId`,`playerObjId`)
) ;