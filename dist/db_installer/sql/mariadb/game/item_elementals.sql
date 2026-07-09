DROP TABLE IF EXISTS `item_elementals`;
CREATE TABLE IF NOT EXISTS `item_elementals` (
  `itemId` int(11) NOT NULL DEFAULT 0,
  `elemType` tinyint(1) NOT NULL DEFAULT -1,
  `elemValue` int(11) NOT NULL DEFAULT -1,
  PRIMARY KEY (`itemId`, `elemType`)
) DEFAULT CHARSET=latin1 COLLATE=latin1_general_ci;

CREATE INDEX idx_itemId_elemType ON item_elementals (itemId, elemType);
