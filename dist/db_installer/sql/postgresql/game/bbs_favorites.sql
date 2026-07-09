DROP TABLE IF EXISTS `bbs_favorites`;
CREATE TABLE IF NOT EXISTS `bbs_favorites` (
	`favId` INT SERIAL,
	`playerId` INT NOT NULL,
	`favTitle` VARCHAR(50) NOT NULL,
	`favBypass` VARCHAR(127) NOT NULL,
	`favAddDate` TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	PRIMARY KEY (`favId`)
) ;
CREATE UNIQUE INDEX IF NOT EXISTS `favId_playerId` ON `bbs_favorites` (`favId`, `playerId`);