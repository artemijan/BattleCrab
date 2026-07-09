DROP TABLE IF EXISTS `item_variables`;
CREATE TABLE IF NOT EXISTS `item_variables` (
  `id` int NOT NULL,
  `var` varchar(255) NOT NULL,
  `val` text NOT NULL,
  KEY `charId` (`id`)
) ;

CREATE INDEX idx_id ON item_variables (id);

CREATE INDEX IF NOT EXISTS `charId` ON `item_variables` (`id`);