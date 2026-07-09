DROP TABLE IF EXISTS `clan_variables`;
CREATE TABLE IF NOT EXISTS `clan_variables` (
  `clanId` int NOT NULL,
  `var` varchar(255) NOT NULL,
  `val` text NOT NULL,
  KEY `clanId` (`clanId`)
) ;
CREATE INDEX IF NOT EXISTS `clanId` ON `clan_variables` (`clanId`);