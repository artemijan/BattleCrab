DROP TABLE IF EXISTS `clanhall`;
CREATE TABLE IF NOT EXISTS `clanhall` (
  `id` int NOT NULL DEFAULT '0',
  `ownerId` int NOT NULL DEFAULT '0',
  `paidUntil` bigint NOT NULL DEFAULT '0',
  PRIMARY KEY `id` (`id`),
  KEY `ownerId` (`ownerId`)
) ;
CREATE INDEX IF NOT EXISTS `id` ON `clanhall` (`id`);
CREATE INDEX IF NOT EXISTS `ownerId` ON `clanhall` (`ownerId`);