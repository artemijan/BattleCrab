DROP TABLE IF EXISTS `itemsonground`;
CREATE TABLE IF NOT EXISTS `itemsonground` (
  `object_id` int NOT NULL DEFAULT '0',
  `item_id` int DEFAULT NULL,
  `count` BIGINT NOT NULL DEFAULT 0,
  `enchant_level` int DEFAULT NULL,
  `x` int DEFAULT NULL,
  `y` int DEFAULT NULL,
  `z` int DEFAULT NULL,
  `drop_time` bigint NOT NULL DEFAULT '0',
  `equipable` int DEFAULT '0',
  PRIMARY KEY (`object_id`)
) ;