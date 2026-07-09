DROP TABLE IF EXISTS `auction_bid`;
CREATE TABLE IF NOT EXISTS `auction_bid` (
  `id` INT NOT NULL DEFAULT 0,
  `auctionId` INT NOT NULL DEFAULT 0,
  `bidderId` INT NOT NULL DEFAULT 0,
  `bidderName` varchar(50) NOT NULL,
  `clan_name` varchar(50) NOT NULL,
  `maxBid` BIGINT NOT NULL DEFAULT 0,
  `time_bid` bigint NOT NULL DEFAULT '0',
  PRIMARY KEY  (`auctionId`,`bidderId`)
) ;
CREATE INDEX IF NOT EXISTS `id` ON `auction_bid` (`id`);