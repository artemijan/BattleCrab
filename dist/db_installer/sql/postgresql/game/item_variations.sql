CREATE TABLE IF NOT EXISTS `item_variations` (
  `itemId` INT NOT NULL,
  `mineralId` INT NOT NULL DEFAULT 0,
  `option1` INT NOT NULL,
  `option2` INT NOT NULL,
  PRIMARY KEY (`itemId`)
) ;

CREATE INDEX idx_itemId ON item_variations (itemId);
