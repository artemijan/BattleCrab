-- `login` is nullable: a row with a NULL login is a dashboard "master" account,
-- identified by `email` instead of a game login name. Game accounts (login NOT
-- NULL) belong to the master account sharing their `email`.
-- `is_verified` is NULL on game accounts, 0/1 on a master account.
DROP TABLE IF EXISTS `accounts`;
CREATE TABLE IF NOT EXISTS `accounts` (
  `login` VARCHAR(45) DEFAULT NULL,
  `password` VARCHAR(45),
  `email` varchar(255) DEFAULT NULL,
  `is_verified` TINYINT DEFAULT NULL,
  `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
  `lastactive` bigint(13) unsigned NOT NULL DEFAULT '0',
  `accessLevel` TINYINT NOT NULL DEFAULT 0,
  `lastIP` CHAR(15) NULL DEFAULT NULL,
  `lastServer` TINYINT DEFAULT 1,
  `pcIp` char(15) DEFAULT NULL,
  `hop1` char(15) DEFAULT NULL,
  `hop2` char(15) DEFAULT NULL,
  `hop3` char(15) DEFAULT NULL,
  `hop4` char(15) DEFAULT NULL,
  UNIQUE KEY `login` (`login`),
  KEY `email` (`email`)
) DEFAULT CHARSET=utf8 COLLATE=utf8_unicode_ci;
-- NOTE: "one master account per address" is a partial-unique constraint
-- (UNIQUE on email WHERE login IS NULL), which MariaDB cannot express. The
-- sqlite and postgresql schemas carry it as a partial index; here it is
-- enforced only by the dashboard API. Keep that check in the application.
