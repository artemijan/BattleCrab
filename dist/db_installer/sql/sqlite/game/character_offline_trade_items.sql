DROP TABLE IF EXISTS `character_offline_trade_items`;
CREATE TABLE IF NOT EXISTS `character_offline_trade_items` (
  `charId` int NOT NULL,
  `item` int NOT NULL DEFAULT '0', -- itemId(for buy) & ObjectId(for sell)
  `count` bigint NOT NULL DEFAULT '0',
  `price` bigint NOT NULL DEFAULT '0'
) ;
CREATE INDEX IF NOT EXISTS `charId` ON `character_offline_trade_items` (`charId`);
CREATE INDEX IF NOT EXISTS `item` ON `character_offline_trade_items` (`item`);