DROP TABLE IF EXISTS `item_auction`;
CREATE TABLE IF NOT EXISTS `item_auction` (
  `auctionId` int NOT NULL,
  `instanceId` int NOT NULL,
  `auctionItemId` int NOT NULL,
  `startingTime` bigint NOT NULL DEFAULT '0',
  `endingTime` bigint NOT NULL DEFAULT '0',
  `auctionStateId` tinyint NOT NULL,
  PRIMARY KEY (`auctionId`)
) ;