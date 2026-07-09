DROP TABLE IF EXISTS `custom_teleport`;
CREATE TABLE IF NOT EXISTS `custom_teleport` (
  `Description` varchar(75) DEFAULT NULL,
  `id` mediumint NOT NULL DEFAULT '0',
  `loc_x` mediumint DEFAULT NULL,
  `loc_y` mediumint DEFAULT NULL,
  `loc_z` mediumint DEFAULT NULL,
  `price` int DEFAULT NULL,
  `fornoble` tinyint NOT NULL DEFAULT '0',
  `itemId` smallint NOT NULL DEFAULT '57',
  PRIMARY KEY (`id`)
) ;