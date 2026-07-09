CREATE TABLE IF NOT EXISTS `characters` (
  `account_name` VARCHAR(45) DEFAULT NULL,
  `charId` INT NOT NULL DEFAULT 0,
  `char_name` VARCHAR(35) NOT NULL,
  `level` TINYINT DEFAULT NULL,
  `maxHp` MEDIUMINT DEFAULT NULL,
  `curHp` MEDIUMINT DEFAULT NULL,
  `maxCp` MEDIUMINT DEFAULT NULL,
  `curCp` MEDIUMINT DEFAULT NULL,
  `maxMp` MEDIUMINT DEFAULT NULL,
  `curMp` MEDIUMINT DEFAULT NULL,
  `face` TINYINT DEFAULT NULL,
  `hairStyle` TINYINT DEFAULT NULL,
  `hairColor` TINYINT DEFAULT NULL,
  `sex` TINYINT DEFAULT NULL,
  `heading` MEDIUMINT DEFAULT NULL,
  `x` MEDIUMINT DEFAULT NULL,
  `y` MEDIUMINT DEFAULT NULL,
  `z` MEDIUMINT DEFAULT NULL,
  `exp` BIGINT DEFAULT 0,
  `expBeforeDeath` BIGINT DEFAULT 0,
  `sp` BIGINT NOT NULL DEFAULT 0,
  `reputation` INT DEFAULT NULL,
  `fame` MEDIUMINT NOT NULL DEFAULT 0,
  `raidbossPoints` MEDIUMINT NOT NULL DEFAULT 0,
  `pvpkills` SMALLINT DEFAULT NULL,
  `pkkills` SMALLINT DEFAULT NULL,
  `clanid` INT DEFAULT NULL,
  `race` TINYINT DEFAULT NULL,
  `classid` TINYINT DEFAULT NULL,
  `base_class` TINYINT NOT NULL DEFAULT 0,
  `transform_id` SMALLINT NOT NULL DEFAULT 0,
  `deletetime` bigint NOT NULL DEFAULT '0',
  `cancraft` TINYINT DEFAULT NULL,
  `title` VARCHAR(21) DEFAULT NULL,
  `title_color` MEDIUMINT NOT NULL DEFAULT 0xECF9A2,
  `accesslevel` MEDIUMINT DEFAULT 0,
  `online` TINYINT DEFAULT NULL,
  `onlinetime` INT DEFAULT NULL,
  `char_slot` TINYINT DEFAULT NULL,
  `lastAccess` bigint NOT NULL DEFAULT '0',
  `clan_privs` INT DEFAULT 0,
  `wantspeace` TINYINT DEFAULT 0,
  `power_grade` TINYINT DEFAULT NULL,
  `nobless` TINYINT NOT NULL DEFAULT 0,
  `subpledge` SMALLINT NOT NULL DEFAULT 0,
  `lvl_joined_academy` TINYINT NOT NULL DEFAULT 0,
  `apprentice` INT NOT NULL DEFAULT 0,
  `sponsor` INT NOT NULL DEFAULT 0,
  `clan_join_expiry_time` bigint NOT NULL DEFAULT '0',
  `clan_create_expiry_time` bigint NOT NULL DEFAULT '0',
  `bookmarkslot` SMALLINT NOT NULL DEFAULT 0,
  `vitality_points` MEDIUMINT NOT NULL DEFAULT 0,
  `createDate` date NOT NULL DEFAULT '2015-01-01',
  `language` VARCHAR(2) DEFAULT NULL,
  `faction` TINYINT NOT NULL DEFAULT '0',
  `pccafe_points` int NOT NULL DEFAULT '0',
  PRIMARY KEY (`charId`),
  KEY `account_name` (`account_name`),
  KEY `char_name` (`char_name`),
  KEY `clanid` (`clanid`),
  KEY `online` (`online`)
) ;

# Common
CREATE INDEX idx_charId ON characters (charId);
CREATE INDEX idx_char_name ON characters (char_name);
CREATE INDEX idx_account_name ON characters (account_name);

# CharSelectionInfo
CREATE INDEX idx_accountName_createDate ON characters (account_name, createDate);

# TaskBirthday
CREATE INDEX idx_createDate ON characters (createDate);

CREATE INDEX IF NOT EXISTS `account_name` ON `characters` (`account_name`);
CREATE INDEX IF NOT EXISTS `char_name` ON `characters` (`char_name`);
CREATE INDEX IF NOT EXISTS `clanid` ON `characters` (`clanid`);
CREATE INDEX IF NOT EXISTS `online` ON `characters` (`online`);