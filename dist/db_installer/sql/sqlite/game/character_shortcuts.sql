DROP TABLE IF EXISTS `character_shortcuts`;
CREATE TABLE IF NOT EXISTS `character_shortcuts` (
  `charId` INT NOT NULL DEFAULT 0,
  `slot` decimal(3) NOT NULL DEFAULT 0,
  `page` decimal(3) NOT NULL DEFAULT 0,
  `type` decimal(3) ,
  `shortcut_id` decimal(16) ,
  `level` MEDIUMINT,
  `sub_level` INT NOT NULL DEFAULT '0',
  `class_index` int NOT NULL DEFAULT '0',
  PRIMARY KEY (`charId`,`slot`,`page`,`class_index`)
) ;

CREATE INDEX IF NOT EXISTS `shortcut_id` ON `character_shortcuts` (`shortcut_id`);