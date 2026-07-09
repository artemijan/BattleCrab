DROP TABLE IF EXISTS `character_premium_items`;
CREATE TABLE IF NOT EXISTS `character_premium_items` (
  `charId` int NOT NULL,
  `itemNum` int NOT NULL,
  `itemId` int NOT NULL,
  `itemCount` bigint NOT NULL,
  `itemSender` varchar(50) NOT NULL,
  KEY `charId` (`charId`),
  KEY `itemNum` (`itemNum`),
  KEY `itemId` (`itemId`)
) ;
CREATE INDEX IF NOT EXISTS `charId` ON `character_premium_items` (`charId`);
CREATE INDEX IF NOT EXISTS `itemNum` ON `character_premium_items` (`itemNum`);
CREATE INDEX IF NOT EXISTS `itemId` ON `character_premium_items` (`itemId`);