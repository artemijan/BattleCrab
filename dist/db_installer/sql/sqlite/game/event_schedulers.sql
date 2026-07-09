DROP TABLE IF EXISTS `event_schedulers`;
CREATE TABLE IF NOT EXISTS `event_schedulers` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT,
  `eventName` varchar(255) NOT NULL,
  `schedulerName` varchar(255) NOT NULL,
  `lastRun` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP
) ;
CREATE UNIQUE INDEX IF NOT EXISTS `eventName_schedulerName_unique` ON `event_schedulers` (`eventName`,`schedulerName`);