DROP TABLE IF EXISTS `punishments`;
CREATE TABLE IF NOT EXISTS `punishments` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT,
  `key` varchar(255) NOT NULL,
  `affect` varchar(255) NOT NULL,
  `type` varchar(255) NOT NULL,
  `expiration`  bigint NOT NULL,
  `reason` TEXT NOT NULL,
  `punishedBy` varchar(255) NOT NULL
) ;