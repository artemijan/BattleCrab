DROP TABLE IF EXISTS `character_item_reuse_save`;
CREATE TABLE IF NOT EXISTS `character_item_reuse_save` (
  `charId` INT NOT NULL DEFAULT 0,
  `itemId` INT NOT NULL DEFAULT 0,
  `itemObjId` INT NOT NULL DEFAULT 1,
  `reuseDelay` INT NOT NULL DEFAULT 0,
  `systime` BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY (`charId`,`itemId`,`itemObjId`)
) ;