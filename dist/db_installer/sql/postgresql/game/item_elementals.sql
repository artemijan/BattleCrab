DROP TABLE IF EXISTS `item_elementals`;
CREATE TABLE IF NOT EXISTS `item_elementals` (
  `itemId` int NOT NULL DEFAULT 0,
  `elemType` tinyint NOT NULL DEFAULT -1,
  `elemValue` int NOT NULL DEFAULT -1,
  PRIMARY KEY (`itemId`, `elemType`)
) ;

CREATE INDEX idx_itemId_elemType ON item_elementals (itemId, elemType);
