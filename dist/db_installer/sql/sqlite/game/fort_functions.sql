DROP TABLE IF EXISTS `fort_functions`;
CREATE TABLE IF NOT EXISTS `fort_functions` (
  `fort_id` int NOT NULL DEFAULT '0',
  `type` int NOT NULL DEFAULT '0',
  `lvl` int NOT NULL DEFAULT '0',
  `lease` int NOT NULL DEFAULT '0',
  `rate` decimal(20,0) NOT NULL DEFAULT '0',
  `endTime` bigint NOT NULL DEFAULT '0',
  PRIMARY KEY (`fort_id`,`type`)
) ;