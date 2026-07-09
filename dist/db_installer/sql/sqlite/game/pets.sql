DROP TABLE IF EXISTS `pets`;
CREATE TABLE IF NOT EXISTS `pets` (
  `item_obj_id` int NOT NULL,
  `name` varchar(16),
  `level` smallint NOT NULL,
  `curHp` int DEFAULT '0',
  `curMp` int DEFAULT '0',
  `exp` bigint DEFAULT '0',
  `sp` bigint DEFAULT '0',
  `fed` int DEFAULT '0',
  `ownerId` int NOT NULL DEFAULT '0',
  `restore` varchar(10) NOT NULL DEFAULT 'false',
  PRIMARY KEY (`item_obj_id`)
) ;
CREATE INDEX IF NOT EXISTS `ownerId` ON `pets` (`ownerId`);