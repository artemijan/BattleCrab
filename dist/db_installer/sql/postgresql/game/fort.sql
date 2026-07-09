DROP TABLE IF EXISTS `fort`;
CREATE TABLE IF NOT EXISTS `fort` (
  `id` int NOT NULL DEFAULT 0,
  `name` varchar(25) NOT NULL,
  `siegeDate` bigint NOT NULL DEFAULT '0',
  `lastOwnedTime` bigint NOT NULL DEFAULT '0',
  `owner` int NOT NULL DEFAULT 0,
  `fortType` int NOT NULL DEFAULT 0,
  `state` int NOT NULL DEFAULT 0,
  `castleId` int NOT NULL DEFAULT 0,
  `supplyLvL` int NOT NULL DEFAULT 0,
  PRIMARY KEY (`id`),
  KEY `owner` (`owner`)
) ;

INSERT IGNORE INTO `fort` VALUES
(101,'Shanty',0,0,0,0,0,0,0),
(102,'Southern',0,0,0,1,0,0,0),
(103,'Hive',0,0,0,0,0,0,0),
(104,'Valley',0,0,0,1,0,0,0),
(105,'Ivory',0,0,0,0,0,0,0),
(106,'Narsell',0,0,0,0,0,0,0),
(107,'Bayou',0,0,0,1,0,0,0),
(108,'White Sands',0,0,0,0,0,0,0),
(109,'Borderland',0,0,0,1,0,0,0),
(110,'Swamp',0,0,0,1,0,0,0),
(111,'Archaic',0,0,0,0,0,0,0),
(112,'Floran',0,0,0,1,0,0,0),
(113,'Cloud Mountain',0,0,0,1,0,0,0),
(114,'Tanor',0,0,0,0,0,0,0),
(115,'Dragonspine',0,0,0,0,0,0,0),
(116,'Antharas',0,0,0,1,0,0,0),
(117,'Western',0,0,0,1,0,0,0),
(118,'Hunters',0,0,0,1,0,0,0),
(119,'Aaru',0,0,0,0,0,0,0),
(120,'Demon',0,0,0,0,0,0,0),
(121,'Monastic',0,0,0,0,0,0,0);
CREATE INDEX IF NOT EXISTS `owner` ON `fort` (`owner`);