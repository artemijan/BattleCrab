DROP TABLE IF EXISTS `character_variables`;
CREATE TABLE IF NOT EXISTS `character_variables` (
  `charId` int NOT NULL,
  `var` varchar(255) NOT NULL,
  `val` text NOT NULL
) ;

CREATE INDEX idx_charId ON character_variables (charId);
CREATE INDEX idx_var ON character_variables (var);
