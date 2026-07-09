DROP TABLE IF EXISTS `clan_notices`;
CREATE TABLE IF NOT EXISTS `clan_notices` (
  `clan_id` INT NOT NULL DEFAULT 0,
  `enabled` varchar(10) DEFAULT 'false' NOT NULL,
  `notice` TEXT NOT NULL,
  PRIMARY KEY  (`clan_id`)
) ;