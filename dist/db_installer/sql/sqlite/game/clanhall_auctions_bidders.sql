DROP TABLE IF EXISTS `clanhall_auctions_bidders`;
CREATE TABLE IF NOT EXISTS `clanhall_auctions_bidders` (
  `clanHallId` INT NOT NULL DEFAULT 0,
  `clanId` INT NOT NULL DEFAULT 0,
  `bid` BIGINT NOT NULL DEFAULT 0,
  `bidTime` BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY( `clanHallId`, `clanId`)
) ;