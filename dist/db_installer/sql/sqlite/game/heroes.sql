DROP TABLE IF EXISTS `heroes`;
CREATE TABLE IF NOT EXISTS `heroes` (
  `charId` INT NOT NULL DEFAULT 0,
  `class_id` decimal(3,0) NOT NULL DEFAULT 0,
  `count` decimal(3,0) NOT NULL DEFAULT 0,
  `played` decimal(1,0) NOT NULL DEFAULT 0,
  `claimed` varchar(5) NOT NULL DEFAULT 'false',
  `message` varchar(300) NOT NULL DEFAULT '',
  PRIMARY KEY (`charId`)
) ;
