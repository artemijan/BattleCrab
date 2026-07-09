DROP TABLE IF EXISTS `character_mentees`;
CREATE TABLE IF NOT EXISTS `character_mentees` (
  `charId` int NOT NULL DEFAULT '0',
  `mentorId` int NOT NULL DEFAULT '0'
) ;