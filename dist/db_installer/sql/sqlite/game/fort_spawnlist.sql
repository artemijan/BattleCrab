DROP TABLE IF EXISTS `fort_spawnlist`;
CREATE TABLE `fort_spawnlist` (
  `fortId` tinyint NOT NULL DEFAULT '0',
  `id` INTEGER PRIMARY KEY AUTOINCREMENT,
  `npcId` smallint NOT NULL DEFAULT '0',
  `x` mediumint NOT NULL DEFAULT '0',
  `y` mediumint NOT NULL DEFAULT '0',
  `z` mediumint NOT NULL DEFAULT '0',
  `heading` mediumint NOT NULL DEFAULT '0',
  `spawnType` tinyint NOT NULL DEFAULT '0', -- 0-always spawned, 1-despawned during siege, 2-despawned 10min before siege, 3-spawned after fort taken
  `castleId` tinyint NOT NULL DEFAULT '0'  -- Castle ID for Special Envoys
) ;
CREATE INDEX IF NOT EXISTS `idx_fortId` ON `fort_spawnlist` (`fortId`);