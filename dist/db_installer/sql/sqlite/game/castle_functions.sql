DROP TABLE IF EXISTS `castle_functions`;
CREATE TABLE IF NOT EXISTS `castle_functions` (
  `castle_id` int NOT NULL DEFAULT '0',
  `type` int NOT NULL DEFAULT '0',
  `lvl` int NOT NULL DEFAULT '0',
  `lease` int NOT NULL DEFAULT '0',
  `rate` decimal(20,0) NOT NULL DEFAULT '0',
  `endTime` bigint NOT NULL DEFAULT '0',
  PRIMARY KEY (`castle_id`,`type`)
) ;