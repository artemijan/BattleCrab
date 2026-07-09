DROP TABLE IF EXISTS `bot_reported_char_data`;
CREATE TABLE IF NOT EXISTS `bot_reported_char_data` (
	`botId` INT NOT NULL DEFAULT 0,
	`reporterId` INT NOT NULL DEFAULT 0,
	`reportDate` BIGINT NOT NULL DEFAULT '0',
	PRIMARY KEY (`botId`, `reporterId`)
) ;