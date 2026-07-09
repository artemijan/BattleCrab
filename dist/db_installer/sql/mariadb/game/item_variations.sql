CREATE TABLE IF NOT EXISTS `item_variations` (
  `itemId` INT(11) NOT NULL,
  `mineralId` INT(11) NOT NULL DEFAULT 0,
  `option1` INT(11) NOT NULL,
  `option2` INT(11) NOT NULL,
  PRIMARY KEY (`itemId`)
) DEFAULT CHARSET=latin1 COLLATE=latin1_general_ci;

CREATE INDEX idx_itemId ON item_variations (itemId);
