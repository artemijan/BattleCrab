-- Migrates an existing SQLite login DB to the master-account schema
-- (PLAN_DASHBOARD.md §15). Run once, against a stopped server, on a backup.
--
--   cp interlude_classic.db interlude_classic.db.bak
--   sqlite3 interlude_classic.db < docs/migrations/2026-07-21-master-accounts.sql
--
-- Deliberately NOT in dist/db_installer/sql/: that tree is fresh-install DDL and
-- every file there starts with DROP TABLE. This one preserves data.
--
-- What changes:
--   * `login` becomes nullable (it can no longer be the primary key, so it
--     becomes a UNIQUE index) — a NULL login marks a dashboard master account.
--   * `is_verified` is added: NULL on game accounts, 0/1 on master accounts.
--   * `accounts_master_email` enforces one master account per address.
--
-- What happens to existing rows: every one of them keeps its login, so they all
-- become *game* accounts (is_verified NULL). Nobody has a master account
-- afterwards. That is intended — the address on an existing row was only ever
-- written by the old verify-link handler, so when its owner registers a master
-- account with that same address, their existing game accounts link to it
-- automatically by the shared address. Anyone else simply registers fresh.

PRAGMA foreign_keys = OFF;

BEGIN TRANSACTION;

CREATE TABLE `accounts_new` (
  `login` VARCHAR(45) DEFAULT NULL,
  `password` VARCHAR(45),
  `email` varchar(255) DEFAULT NULL,
  `is_verified` TINYINT DEFAULT NULL,
  `created_time` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
  `lastactive` bigint NOT NULL DEFAULT '0',
  `accessLevel` TINYINT NOT NULL DEFAULT 0,
  `lastIP` CHAR(15) NULL DEFAULT NULL,
  `lastServer` TINYINT DEFAULT 1,
  `pcIp` char(15) DEFAULT NULL,
  `hop1` char(15) DEFAULT NULL,
  `hop2` char(15) DEFAULT NULL,
  `hop3` char(15) DEFAULT NULL,
  `hop4` char(15) DEFAULT NULL,
  UNIQUE (`login`)
);

INSERT INTO `accounts_new`
  (`login`, `password`, `email`, `is_verified`, `created_time`, `lastactive`,
   `accessLevel`, `lastIP`, `lastServer`, `pcIp`, `hop1`, `hop2`, `hop3`, `hop4`)
SELECT
  `login`, `password`, `email`, NULL, `created_time`, `lastactive`,
  `accessLevel`, `lastIP`, `lastServer`, `pcIp`, `hop1`, `hop2`, `hop3`, `hop4`
FROM `accounts`;

DROP TABLE `accounts`;
ALTER TABLE `accounts_new` RENAME TO `accounts`;

CREATE UNIQUE INDEX IF NOT EXISTS `accounts_master_email`
  ON `accounts` (`email` COLLATE NOCASE) WHERE `login` IS NULL;

COMMIT;

PRAGMA foreign_keys = ON;
