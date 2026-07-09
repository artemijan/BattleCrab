DROP TABLE IF EXISTS `siege_clans`;
CREATE TABLE IF NOT EXISTS `siege_clans` (
   `castle_id` int NOT NULL DEFAULT 0,
   `clan_id` int NOT NULL DEFAULT 0,
   `type` int DEFAULT NULL,
   `castle_owner` int DEFAULT NULL,
   PRIMARY KEY (`clan_id`,`castle_id`)
) ;